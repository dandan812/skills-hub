use rusqlite::{params, Connection, Error, Result, Transaction};
use serde::Serialize;

pub const MAX_RUNTIME_EVENTS: usize = 10_000;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS runtime_evidence_events (
    event_id TEXT PRIMARY KEY CHECK (
        length(event_id) BETWEEN 1 AND 128
        AND event_id NOT GLOB '*[^A-Za-z0-9._:-]*'
    ),
    agent_id TEXT NOT NULL CHECK (agent_id IN ('codex', 'claude', 'opencode', 'pi')),
    session_id TEXT NOT NULL CHECK (
        length(session_id) BETWEEN 1 AND 128
        AND session_id NOT GLOB '*[^A-Za-z0-9._:-]*'
    ),
    skill_id TEXT CHECK (
        skill_id IS NULL OR (
            length(skill_id) BETWEEN 1 AND 64
            AND skill_id GLOB '[a-z0-9]*'
            AND skill_id NOT GLOB '*[^a-z0-9._-]*'
        )
    ),
    event_type TEXT NOT NULL CHECK (event_type IN (
        'session.started', 'skill.called', 'skill.loaded',
        'context.compacted', 'session.ended'
    )),
    observed_at TEXT NOT NULL CHECK (
        length(observed_at) BETWEEN 20 AND 35
        AND observed_at GLOB '*Z'
        AND observed_at NOT GLOB '*[^0-9T:.Z-]*'
    ),
    imported_at TEXT NOT NULL CHECK (
        length(imported_at) BETWEEN 20 AND 35
        AND imported_at GLOB '*Z'
        AND imported_at NOT GLOB '*[^0-9T:.Z-]*'
    ),
    content_hash TEXT NOT NULL CHECK (
        length(content_hash) = 64
        AND content_hash NOT GLOB '*[^0-9a-f]*'
    ),
    CHECK (
        (event_type IN ('skill.called', 'skill.loaded') AND skill_id IS NOT NULL)
        OR (event_type IN ('session.started', 'context.compacted', 'session.ended') AND skill_id IS NULL)
    )
);
CREATE INDEX IF NOT EXISTS runtime_evidence_agent_session_observed_idx
ON runtime_evidence_events (agent_id, session_id, observed_at, event_id);
CREATE TRIGGER IF NOT EXISTS runtime_evidence_no_update
BEFORE UPDATE ON runtime_evidence_events
BEGIN SELECT RAISE(ABORT, 'runtime evidence is append-only'); END;
CREATE TRIGGER IF NOT EXISTS runtime_evidence_no_delete
BEFORE DELETE ON runtime_evidence_events
BEGIN SELECT RAISE(ABORT, 'runtime evidence is append-only'); END;
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeAgent {
    Codex,
    Claude,
    Opencode,
    Pi,
}

impl RuntimeAgent {
    pub fn value(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Opencode => "opencode",
            Self::Pi => "pi",
        }
    }

    pub fn tool_id(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude_code",
            Self::Opencode => "opencode",
            Self::Pi => "pi",
        }
    }

    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            "codex" => Some(Self::Codex),
            "claude" => Some(Self::Claude),
            "opencode" => Some(Self::Opencode),
            "pi" => Some(Self::Pi),
            _ => None,
        }
    }

    pub const fn all() -> [Self; 4] {
        [Self::Codex, Self::Claude, Self::Opencode, Self::Pi]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeEventType {
    SessionStarted,
    SkillCalled,
    SkillLoaded,
    ContextCompacted,
    SessionEnded,
}

impl RuntimeEventType {
    pub fn value(self) -> &'static str {
        match self {
            Self::SessionStarted => "session.started",
            Self::SkillCalled => "skill.called",
            Self::SkillLoaded => "skill.loaded",
            Self::ContextCompacted => "context.compacted",
            Self::SessionEnded => "session.ended",
        }
    }

    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            "session.started" => Some(Self::SessionStarted),
            "skill.called" => Some(Self::SkillCalled),
            "skill.loaded" => Some(Self::SkillLoaded),
            "context.compacted" => Some(Self::ContextCompacted),
            "session.ended" => Some(Self::SessionEnded),
            _ => None,
        }
    }

    pub fn requires_skill(self) -> bool {
        matches!(self, Self::SkillCalled | Self::SkillLoaded)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEvent {
    pub event_id: String,
    pub agent_id: RuntimeAgent,
    pub session_id: String,
    pub skill_id: Option<String>,
    pub event_type: RuntimeEventType,
    pub observed_at: String,
    pub imported_at: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppendResult {
    Inserted,
    Duplicate,
}

#[derive(Debug)]
pub enum AppendError {
    Database,
    InvalidEvent,
    ConflictingEventId,
}

impl From<Error> for AppendError {
    fn from(_: Error) -> Self {
        Self::Database
    }
}

pub fn ensure_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(SCHEMA)
}

pub fn append_in_transaction(
    transaction: &Transaction<'_>,
    event: &RuntimeEvent,
) -> std::result::Result<AppendResult, AppendError> {
    if !is_safe_event(event) {
        return Err(AppendError::InvalidEvent);
    }
    let inserted = transaction.execute(
        "INSERT INTO runtime_evidence_events
         (event_id, agent_id, session_id, skill_id, event_type, observed_at, imported_at, content_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            event.event_id,
            event.agent_id.value(),
            event.session_id,
            event.skill_id,
            event.event_type.value(),
            event.observed_at,
            event.imported_at,
            event.content_hash,
        ],
    );
    match inserted {
        Ok(1) => Ok(AppendResult::Inserted),
        Ok(_) => Err(AppendError::Database),
        Err(error)
            if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation) =>
        {
            let existing_hash: String = transaction.query_row(
                "SELECT content_hash FROM runtime_evidence_events WHERE event_id = ?1",
                [&event.event_id],
                |row| row.get(0),
            )?;
            if existing_hash == event.content_hash {
                Ok(AppendResult::Duplicate)
            } else {
                Err(AppendError::ConflictingEventId)
            }
        }
        Err(_) => Err(AppendError::Database),
    }
}

pub fn event_content_hash(connection: &Connection, event_id: &str) -> Result<Option<String>> {
    match connection.query_row(
        "SELECT content_hash FROM runtime_evidence_events WHERE event_id = ?1",
        [event_id],
        |row| row.get(0),
    ) {
        Ok(hash) => Ok(Some(hash)),
        Err(Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(error),
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SessionLifecycle {
    pub started: bool,
    pub ended: bool,
}

pub fn session_lifecycle(
    connection: &Connection,
    agent: RuntimeAgent,
    session_id: &str,
) -> Result<SessionLifecycle> {
    connection.query_row(
        "SELECT
           COALESCE(MAX(CASE WHEN event_type = 'session.started' THEN 1 ELSE 0 END), 0),
           COALESCE(MAX(CASE WHEN event_type = 'session.ended' THEN 1 ELSE 0 END), 0)
         FROM runtime_evidence_events WHERE agent_id = ?1 AND session_id = ?2",
        params![agent.value(), session_id],
        |row| {
            Ok(SessionLifecycle {
                started: row.get::<_, i64>(0)? != 0,
                ended: row.get::<_, i64>(1)? != 0,
            })
        },
    )
}

pub fn list_events(connection: &Connection) -> Result<Vec<RuntimeEvent>> {
    let mut statement = connection.prepare(
        "SELECT event_id, agent_id, session_id, skill_id, event_type, observed_at, imported_at, content_hash
         FROM (
           SELECT event_id, agent_id, session_id, skill_id, event_type, observed_at, imported_at, content_hash
           FROM runtime_evidence_events ORDER BY observed_at DESC, event_id DESC LIMIT ?1
         ) ORDER BY observed_at ASC, event_id ASC",
    )?;
    let events = statement
        .query_map([MAX_RUNTIME_EVENTS as i64], |row| {
            let agent: String = row.get(1)?;
            let event_type: String = row.get(4)?;
            Ok(RuntimeEvent {
                event_id: row.get(0)?,
                agent_id: RuntimeAgent::from_value(&agent).ok_or(Error::InvalidQuery)?,
                session_id: row.get(2)?,
                skill_id: row.get(3)?,
                event_type: RuntimeEventType::from_value(&event_type).ok_or(Error::InvalidQuery)?,
                observed_at: row.get(5)?,
                imported_at: row.get(6)?,
                content_hash: row.get(7)?,
            })
        })?
        .collect();
    events
}

fn is_safe_event(event: &RuntimeEvent) -> bool {
    safe_id(&event.event_id, 128)
        && safe_id(&event.session_id, 128)
        && safe_timestamp(&event.observed_at)
        && safe_timestamp(&event.imported_at)
        && event.content_hash.len() == 64
        && event
            .content_hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && match (&event.skill_id, event.event_type.requires_skill()) {
            (Some(skill_id), true) => safe_skill(skill_id),
            (None, false) => true,
            _ => false,
        }
}

pub fn safe_id(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}

pub fn safe_skill(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn safe_timestamp(value: &str) -> bool {
    (20..=35).contains(&value.len())
        && value.ends_with('Z')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'T' | b':' | b'.' | b'Z' | b'-'))
}
