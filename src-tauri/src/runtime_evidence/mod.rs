pub mod commands;

use serde::{Deserialize, Serialize};

pub const RUNTIME_EVIDENCE_SCHEMA_VERSION: u16 = 1;
pub const RUNTIME_EVIDENCE_EVENT_NAME: &str = "runtime-evidence://event-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEvidenceEventKind {
    SessionStarted,
    SkillLoaded,
    SkillCalled,
}

impl RuntimeEvidenceEventKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::SessionStarted => "session_started",
            Self::SkillLoaded => "skill_loaded",
            Self::SkillCalled => "skill_called",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeEvidenceEventV1 {
    pub schema_version: u16,
    pub event_id: String,
    pub occurred_at_ms: i64,
    pub agent_id: String,
    pub session_id: String,
    pub skill_id: Option<String>,
    pub event_type: RuntimeEvidenceEventKind,
    pub source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEvidenceCollectorState {
    NotConfigured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeEvidenceStatus {
    pub schema_version: u16,
    pub event_name: &'static str,
    pub collector_state: RuntimeEvidenceCollectorState,
    pub last_event_at_ms: Option<i64>,
    pub supported_event_types: Vec<&'static str>,
}

pub fn current_status() -> RuntimeEvidenceStatus {
    RuntimeEvidenceStatus {
        schema_version: RUNTIME_EVIDENCE_SCHEMA_VERSION,
        event_name: RUNTIME_EVIDENCE_EVENT_NAME,
        collector_state: RuntimeEvidenceCollectorState::NotConfigured,
        last_event_at_ms: None,
        supported_event_types: [
            RuntimeEvidenceEventKind::SessionStarted,
            RuntimeEvidenceEventKind::SkillLoaded,
            RuntimeEvidenceEventKind::SkillCalled,
        ]
        .into_iter()
        .map(RuntimeEvidenceEventKind::as_str)
        .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_status_is_truthfully_unconfigured() {
        let status = current_status();

        assert_eq!(status.schema_version, 1);
        assert_eq!(
            status.collector_state,
            RuntimeEvidenceCollectorState::NotConfigured
        );
        assert_eq!(status.last_event_at_ms, None);
        assert_eq!(
            status.supported_event_types,
            vec!["session_started", "skill_loaded", "skill_called"]
        );
    }

    #[test]
    fn event_v1_serialization_is_stable() {
        let event = RuntimeEvidenceEventV1 {
            schema_version: 1,
            event_id: "evt-1".to_string(),
            occurred_at_ms: 1_725_000_000_000,
            agent_id: "codex".to_string(),
            session_id: "session-1".to_string(),
            skill_id: Some("git-essentials".to_string()),
            event_type: RuntimeEvidenceEventKind::SkillCalled,
            source: "local_hook".to_string(),
        };

        let json = serde_json::to_value(event).expect("serialize runtime evidence event");

        assert_eq!(json["schema_version"], 1);
        assert_eq!(json["event_type"], "skill_called");
        assert_eq!(json["skill_id"], "git-essentials");
    }
}
