use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use time::{format_description::well_known::Rfc3339, OffsetDateTime, UtcOffset};

use crate::core::skill_store::SkillStore;

use super::{
    import::RuntimeImportSummary,
    store::{safe_skill, RuntimeAgent, RuntimeEvent, RuntimeEventType},
};

const MAX_SESSIONS_PER_AGENT: usize = 32;
const MAX_SKILLS_PER_SESSION: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OverviewStatus {
    Ready,
    Partial,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CatalogState {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TruthState {
    Yes,
    No,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionState {
    Active,
    Ended,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EvidenceSource {
    ManagementCatalog,
    LocalHookReported,
    CatalogAndLocalHookReported,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EvidenceReason {
    EvidenceAccepted,
    CatalogUnavailable,
    SessionEvidenceMissing,
    LoadEvidenceMissing,
    ContextCompacted,
    SessionEnded,
    InvalidLifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum LastCallState {
    Unknown,
    Observed {
        #[serde(rename = "observedAt")]
        observed_at: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSkillRow {
    pub skill_id: String,
    pub skill_name: String,
    pub installed: TruthState,
    pub assigned: TruthState,
    pub loaded: TruthState,
    pub last_call: LastCallState,
    pub source: EvidenceSource,
    pub reason: EvidenceReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSessionView {
    pub session_id: Option<String>,
    pub state: SessionState,
    pub started_at: Option<String>,
    pub last_observed_at: Option<String>,
    pub skills: Vec<RuntimeSkillRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAgentView {
    pub agent_id: RuntimeAgent,
    pub catalog_state: CatalogState,
    pub sessions: Vec<RuntimeSessionView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOverview {
    pub schema_version: u8,
    pub status: OverviewStatus,
    pub import: RuntimeImportSummary,
    pub inbox_path: Option<String>,
    pub event_count: usize,
    pub agents: Vec<RuntimeAgentView>,
}

#[derive(Default)]
pub struct CatalogSnapshot {
    available: bool,
    installed: BTreeSet<String>,
    names: BTreeMap<String, String>,
    assigned: BTreeMap<RuntimeAgent, BTreeSet<String>>,
}

impl CatalogSnapshot {
    pub fn load(store: &SkillStore) -> anyhow::Result<Self> {
        let skills = store.list_skills()?;
        let mut snapshot = Self {
            available: true,
            ..Self::default()
        };
        for skill in skills {
            let runtime_id = if safe_skill(&skill.name) {
                skill.name.clone()
            } else {
                skill.id.clone()
            };
            snapshot.installed.insert(runtime_id.clone());
            snapshot.names.insert(runtime_id.clone(), skill.name);
            for target in store.list_skill_targets(&skill.id)? {
                if target.status != "ok" {
                    continue;
                }
                if let Some(agent) = RuntimeAgent::all()
                    .into_iter()
                    .find(|agent| agent.tool_id() == target.tool)
                {
                    snapshot
                        .assigned
                        .entry(agent)
                        .or_default()
                        .insert(runtime_id.clone());
                }
            }
        }
        Ok(snapshot)
    }

    pub fn unavailable() -> Self {
        Self::default()
    }

    fn state(&self) -> CatalogState {
        if self.available {
            CatalogState::Available
        } else {
            CatalogState::Unavailable
        }
    }

    fn installed(&self, skill_id: &str) -> TruthState {
        if !self.available {
            TruthState::Unknown
        } else if self.installed.contains(skill_id) {
            TruthState::Yes
        } else {
            TruthState::No
        }
    }

    fn assigned(&self, agent: RuntimeAgent, skill_id: &str) -> TruthState {
        if !self.available {
            TruthState::Unknown
        } else if self
            .assigned
            .get(&agent)
            .is_some_and(|skills| skills.contains(skill_id))
        {
            TruthState::Yes
        } else {
            TruthState::No
        }
    }

    fn name(&self, skill_id: &str) -> String {
        self.names
            .get(skill_id)
            .cloned()
            .unwrap_or_else(|| skill_id.to_owned())
    }
}

#[derive(Default)]
struct SessionFold {
    started: bool,
    ended: bool,
    invalid: bool,
    started_at: Option<String>,
    last_observed_at: Option<String>,
    compacted: bool,
    loaded: BTreeSet<String>,
    calls: BTreeMap<String, String>,
    event_skills: BTreeSet<String>,
}

pub fn project_overview(
    catalog: &CatalogSnapshot,
    events: &[RuntimeEvent],
    import: RuntimeImportSummary,
    inbox_path: Option<String>,
) -> RuntimeOverview {
    let mut partial = false;
    let mut ordered = Vec::new();
    for event in events {
        if let Some(observed_at) = parsed_utc(&event.observed_at) {
            ordered.push((observed_at, event));
        } else {
            partial = true;
        }
    }
    ordered.sort_by(|left, right| {
        (left.0, left.1.event_id.as_str()).cmp(&(right.0, right.1.event_id.as_str()))
    });

    let mut folds = BTreeMap::<(RuntimeAgent, String), SessionFold>::new();
    for (_, event) in ordered {
        let fold = folds
            .entry((event.agent_id, event.session_id.clone()))
            .or_default();
        if fold.invalid {
            continue;
        }
        if fold.ended
            || (fold.started && event.event_type == RuntimeEventType::SessionStarted)
            || (!fold.started && event.event_type != RuntimeEventType::SessionStarted)
        {
            fold.invalid = true;
            fold.loaded.clear();
            fold.calls.clear();
            partial = true;
            continue;
        }
        fold.last_observed_at = Some(event.observed_at.clone());
        match event.event_type {
            RuntimeEventType::SessionStarted => {
                fold.started = true;
                fold.started_at = Some(event.observed_at.clone());
            }
            RuntimeEventType::SkillCalled => {
                if let Some(skill) = &event.skill_id {
                    fold.event_skills.insert(skill.clone());
                    fold.calls.insert(skill.clone(), event.observed_at.clone());
                }
            }
            RuntimeEventType::SkillLoaded => {
                if let Some(skill) = &event.skill_id {
                    fold.event_skills.insert(skill.clone());
                    fold.loaded.insert(skill.clone());
                }
            }
            RuntimeEventType::ContextCompacted => {
                fold.loaded.clear();
                fold.compacted = true;
            }
            RuntimeEventType::SessionEnded => {
                fold.ended = true;
                fold.loaded.clear();
            }
        }
    }

    let mut known_skills = catalog.installed.clone();
    for fold in folds.values() {
        known_skills.extend(fold.event_skills.iter().cloned());
    }
    let mut agents = Vec::with_capacity(4);
    for agent in RuntimeAgent::all() {
        let mut sessions = folds
            .iter()
            .filter(|((candidate, _), _)| *candidate == agent)
            .map(|((_, session_id), fold)| {
                session_view(
                    agent,
                    session_id,
                    fold,
                    catalog,
                    &known_skills,
                    &mut partial,
                )
            })
            .collect::<Vec<_>>();
        sessions.sort_by(|left, right| {
            right
                .last_observed_at
                .cmp(&left.last_observed_at)
                .then_with(|| left.session_id.cmp(&right.session_id))
        });
        if sessions.len() > MAX_SESSIONS_PER_AGENT {
            sessions.truncate(MAX_SESSIONS_PER_AGENT);
            partial = true;
        }
        if sessions.is_empty() {
            sessions.push(unknown_session(agent, catalog, &known_skills, &mut partial));
        }
        agents.push(RuntimeAgentView {
            agent_id: agent,
            catalog_state: catalog.state(),
            sessions,
        });
    }

    let status = if !catalog.available && events.is_empty() {
        OverviewStatus::Unknown
    } else if partial || !catalog.available {
        OverviewStatus::Partial
    } else {
        OverviewStatus::Ready
    };
    RuntimeOverview {
        schema_version: super::RUNTIME_EVIDENCE_SCHEMA_VERSION,
        status,
        import,
        inbox_path,
        event_count: events.len(),
        agents,
    }
}

fn session_view(
    agent: RuntimeAgent,
    session_id: &str,
    fold: &SessionFold,
    catalog: &CatalogSnapshot,
    known_skills: &BTreeSet<String>,
    partial: &mut bool,
) -> RuntimeSessionView {
    let skills = bounded_rows(agent, Some(fold), catalog, known_skills, partial);
    RuntimeSessionView {
        session_id: Some(session_id.to_owned()),
        state: if fold.invalid {
            SessionState::Unknown
        } else if fold.ended {
            SessionState::Ended
        } else {
            SessionState::Active
        },
        started_at: fold.started_at.clone(),
        last_observed_at: fold.last_observed_at.clone(),
        skills,
    }
}

fn unknown_session(
    agent: RuntimeAgent,
    catalog: &CatalogSnapshot,
    known_skills: &BTreeSet<String>,
    partial: &mut bool,
) -> RuntimeSessionView {
    RuntimeSessionView {
        session_id: None,
        state: SessionState::Unknown,
        started_at: None,
        last_observed_at: None,
        skills: bounded_rows(agent, None, catalog, known_skills, partial),
    }
}

fn bounded_rows(
    agent: RuntimeAgent,
    fold: Option<&SessionFold>,
    catalog: &CatalogSnapshot,
    known_skills: &BTreeSet<String>,
    partial: &mut bool,
) -> Vec<RuntimeSkillRow> {
    let mut skills = known_skills
        .iter()
        .map(|skill_id| row(agent, skill_id, fold, catalog))
        .collect::<Vec<_>>();
    if skills.len() > MAX_SKILLS_PER_SESSION {
        skills.truncate(MAX_SKILLS_PER_SESSION);
        *partial = true;
    }
    skills
}

fn row(
    agent: RuntimeAgent,
    skill_id: &str,
    fold: Option<&SessionFold>,
    catalog: &CatalogSnapshot,
) -> RuntimeSkillRow {
    let installed = catalog.installed(skill_id);
    let assigned = catalog.assigned(agent, skill_id);
    let (loaded, last_call, runtime_evidence, reason) = match fold {
        None => (
            TruthState::Unknown,
            LastCallState::Unknown,
            false,
            if installed == TruthState::Unknown && assigned == TruthState::Unknown {
                EvidenceReason::CatalogUnavailable
            } else {
                EvidenceReason::SessionEvidenceMissing
            },
        ),
        Some(fold) if fold.invalid => (
            TruthState::Unknown,
            LastCallState::Unknown,
            false,
            EvidenceReason::InvalidLifecycle,
        ),
        Some(fold) => {
            let loaded = if !fold.ended && fold.loaded.contains(skill_id) {
                TruthState::Yes
            } else {
                TruthState::Unknown
            };
            let last_call = fold
                .calls
                .get(skill_id)
                .cloned()
                .map(|observed_at| LastCallState::Observed { observed_at })
                .unwrap_or(LastCallState::Unknown);
            let has_runtime =
                loaded == TruthState::Yes || matches!(last_call, LastCallState::Observed { .. });
            let reason = if fold.ended {
                EvidenceReason::SessionEnded
            } else if has_runtime {
                EvidenceReason::EvidenceAccepted
            } else if fold.compacted {
                EvidenceReason::ContextCompacted
            } else {
                EvidenceReason::LoadEvidenceMissing
            };
            (loaded, last_call, has_runtime, reason)
        }
    };
    let catalog_evidence = installed != TruthState::Unknown || assigned != TruthState::Unknown;
    let source = match (catalog_evidence, runtime_evidence) {
        (true, true) => EvidenceSource::CatalogAndLocalHookReported,
        (true, false) => EvidenceSource::ManagementCatalog,
        (false, true) => EvidenceSource::LocalHookReported,
        (false, false) => EvidenceSource::Unknown,
    };
    RuntimeSkillRow {
        skill_id: skill_id.to_owned(),
        skill_name: catalog.name(skill_id),
        installed,
        assigned,
        loaded,
        last_call,
        source,
        reason,
    }
}

fn parsed_utc(value: &str) -> Option<OffsetDateTime> {
    let parsed = OffsetDateTime::parse(value, &Rfc3339).ok()?;
    (value.ends_with('Z') && parsed.offset() == UtcOffset::UTC).then_some(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(
        event_type: RuntimeEventType,
        skill_id: Option<&str>,
        observed_at: &str,
    ) -> RuntimeEvent {
        RuntimeEvent {
            event_id: format!("event-{}", observed_at.replace([':', '-'], "")),
            agent_id: RuntimeAgent::Codex,
            session_id: "session-1".to_owned(),
            skill_id: skill_id.map(str::to_owned),
            event_type,
            observed_at: observed_at.to_owned(),
            imported_at: observed_at.to_owned(),
            content_hash: "a".repeat(64),
        }
    }

    #[test]
    fn call_does_not_imply_loaded_and_compaction_clears_loaded() {
        let events = vec![
            event(
                RuntimeEventType::SessionStarted,
                None,
                "2026-08-30T00:00:00Z",
            ),
            event(
                RuntimeEventType::SkillLoaded,
                Some("code"),
                "2026-08-30T00:00:01Z",
            ),
            event(
                RuntimeEventType::SkillCalled,
                Some("review"),
                "2026-08-30T00:00:02Z",
            ),
            event(
                RuntimeEventType::ContextCompacted,
                None,
                "2026-08-30T00:00:03Z",
            ),
        ];
        let overview = project_overview(
            &CatalogSnapshot::unavailable(),
            &events,
            RuntimeImportSummary::empty(),
            None,
        );
        let rows = &overview.agents[0].sessions[0].skills;
        let code = rows.iter().find(|row| row.skill_id == "code").unwrap();
        let review = rows.iter().find(|row| row.skill_id == "review").unwrap();

        assert_eq!(code.loaded, TruthState::Unknown);
        assert_eq!(review.loaded, TruthState::Unknown);
        assert!(matches!(review.last_call, LastCallState::Observed { .. }));
    }
}
