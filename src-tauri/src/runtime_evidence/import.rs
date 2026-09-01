use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, BufReader, Read},
    path::{Path, PathBuf},
};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};

use super::store::{
    append_in_transaction, event_content_hash, safe_id, safe_skill, session_lifecycle, AppendError,
    AppendResult, RuntimeAgent, RuntimeEvent, RuntimeEventType,
};

pub const MAX_INBOX_BYTES: u64 = 5 * 1024 * 1024;
pub const MAX_INBOX_LINES: usize = 10_000;
pub const MAX_INBOX_LINE_BYTES: usize = 4 * 1024;
const INBOX_DIRECTORY: &str = "runtime-hooks";
const INBOX_FILENAME: &str = "skill-runtime-v1.jsonl";
const MAX_EVENT_AGE: Duration = Duration::days(30);
const MAX_FUTURE_SKEW: Duration = Duration::minutes(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeImportReason {
    InboxUnavailable,
    InboxPathRejected,
    InboxTooLarge,
    TooManyLines,
    LineTooLarge,
    CrOnlyFraming,
    InvalidJson,
    InvalidEnvelope,
    InvalidTimestamp,
    TimestampTooOld,
    TimestampTooFarInFuture,
    MissingSessionStarted,
    DuplicateSessionStarted,
    EventAfterSessionEnded,
    ConflictingEventId,
    StorageUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeImportSummary {
    pub accepted: u32,
    pub duplicate: u32,
    pub rejected: u32,
    pub reasons: Vec<RuntimeImportReason>,
}

impl RuntimeImportSummary {
    pub const fn empty() -> Self {
        Self {
            accepted: 0,
            duplicate: 0,
            rejected: 0,
            reasons: Vec::new(),
        }
    }

    pub fn storage_unavailable() -> Self {
        Self::file_failure(RuntimeImportReason::StorageUnavailable)
    }

    fn file_failure(reason: RuntimeImportReason) -> Self {
        Self {
            accepted: 0,
            duplicate: 0,
            rejected: 1,
            reasons: vec![reason],
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeInbox {
    app_local_root: PathBuf,
}

impl RuntimeInbox {
    pub fn from_app_local_root(app_local_root: impl Into<PathBuf>) -> Self {
        Self {
            app_local_root: app_local_root.into(),
        }
    }

    pub fn path(&self) -> PathBuf {
        self.app_local_root
            .join(INBOX_DIRECTORY)
            .join(INBOX_FILENAME)
    }

    fn ensure_owned_inbox(&self) -> Result<PathBuf, RuntimeImportReason> {
        ensure_safe_directory(&self.app_local_root)?;
        let parent = self.app_local_root.join(INBOX_DIRECTORY);
        ensure_safe_directory(&parent)?;
        let inbox = self.path();
        match fs::symlink_metadata(&inbox) {
            Ok(metadata) if is_link_or_reparse(&metadata) || !metadata.is_file() => {
                return Err(RuntimeImportReason::InboxPathRejected)
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&inbox)
                    .map_err(|_| RuntimeImportReason::InboxUnavailable)?;
            }
            Err(_) => return Err(RuntimeImportReason::InboxUnavailable),
        }
        let metadata =
            fs::symlink_metadata(&inbox).map_err(|_| RuntimeImportReason::InboxUnavailable)?;
        if is_link_or_reparse(&metadata) || !metadata.is_file() {
            Err(RuntimeImportReason::InboxPathRejected)
        } else {
            Ok(inbox)
        }
    }
}

pub fn import_inbox(connection: &mut Connection, inbox: &RuntimeInbox) -> RuntimeImportSummary {
    import_inbox_at(connection, inbox, OffsetDateTime::now_utc())
}

fn import_inbox_at(
    connection: &mut Connection,
    inbox: &RuntimeInbox,
    now: OffsetDateTime,
) -> RuntimeImportSummary {
    let inbox_path = match inbox.ensure_owned_inbox() {
        Ok(path) => path,
        Err(reason) => return RuntimeImportSummary::file_failure(reason),
    };
    let metadata = match fs::metadata(&inbox_path) {
        Ok(metadata) => metadata,
        Err(_) => return RuntimeImportSummary::file_failure(RuntimeImportReason::InboxUnavailable),
    };
    if metadata.len() > MAX_INBOX_BYTES {
        return RuntimeImportSummary::file_failure(RuntimeImportReason::InboxTooLarge);
    }
    let lines = match scan_file(&inbox_path, now) {
        Ok(lines) => lines,
        Err(reason) => return RuntimeImportSummary::file_failure(reason),
    };
    classify_and_append(connection, lines, now)
}

fn scan_file(path: &Path, now: OffsetDateTime) -> Result<Vec<LineResult>, RuntimeImportReason> {
    let file = File::open(path).map_err(|_| RuntimeImportReason::InboxUnavailable)?;
    let mut reader = BufReader::new(file);
    let mut total_bytes = 0_u64;
    let mut line_count = 0_usize;
    let mut lines = Vec::new();
    while let Some((line, terminated)) = read_bounded_line(&mut reader, &mut total_bytes)? {
        line_count += 1;
        if line_count > MAX_INBOX_LINES {
            return Err(RuntimeImportReason::TooManyLines);
        }
        if line.len() > MAX_INBOX_LINE_BYTES {
            lines.push(LineResult::Rejected(RuntimeImportReason::LineTooLarge));
            continue;
        }
        if !terminated && line.last() == Some(&b'\r') {
            return Err(RuntimeImportReason::CrOnlyFraming);
        }
        let line = if terminated && line.last() == Some(&b'\r') {
            &line[..line.len() - 1]
        } else {
            &line
        };
        lines.push(match parse_event(line, now) {
            Ok(event) => LineResult::Candidate(event),
            Err(reason) => LineResult::Rejected(reason),
        });
    }
    Ok(lines)
}

fn read_bounded_line(
    reader: &mut BufReader<File>,
    total_bytes: &mut u64,
) -> Result<Option<(Vec<u8>, bool)>, RuntimeImportReason> {
    let mut line = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        match reader.read(&mut byte) {
            Ok(0) if line.is_empty() => return Ok(None),
            Ok(0) => return Ok(Some((line, false))),
            Ok(_) => {
                *total_bytes += 1;
                if *total_bytes > MAX_INBOX_BYTES {
                    return Err(RuntimeImportReason::InboxTooLarge);
                }
                if byte[0] == b'\n' {
                    return Ok(Some((line, true)));
                }
                if line.len() <= MAX_INBOX_LINE_BYTES {
                    line.push(byte[0]);
                }
            }
            Err(_) => return Err(RuntimeImportReason::InboxUnavailable),
        }
    }
}

#[derive(Debug)]
enum LineResult {
    Candidate(NormalizedEvent),
    Rejected(RuntimeImportReason),
}

#[derive(Debug, Clone)]
struct NormalizedEvent {
    event_id: String,
    agent_id: RuntimeAgent,
    session_id: String,
    skill_id: Option<String>,
    event_type: RuntimeEventType,
    observed_at: String,
    content_hash: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawEvent {
    schema_version: u8,
    event_id: String,
    agent: String,
    session_id: String,
    event: String,
    #[serde(default)]
    skill: SkillPresence,
    observed_at: String,
}

#[derive(Debug, Default)]
enum SkillPresence {
    #[default]
    Absent,
    Present(Option<String>),
}

impl<'de> Deserialize<'de> for SkillPresence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer).map(Self::Present)
    }
}

fn parse_event(line: &[u8], now: OffsetDateTime) -> Result<NormalizedEvent, RuntimeImportReason> {
    let raw: RawEvent = serde_json::from_slice(line).map_err(|error| {
        if error.classify() == serde_json::error::Category::Data {
            RuntimeImportReason::InvalidEnvelope
        } else {
            RuntimeImportReason::InvalidJson
        }
    })?;
    if raw.schema_version != 1 || !safe_id(&raw.event_id, 128) || !safe_id(&raw.session_id, 128) {
        return Err(RuntimeImportReason::InvalidEnvelope);
    }
    let agent_id =
        RuntimeAgent::from_value(&raw.agent).ok_or(RuntimeImportReason::InvalidEnvelope)?;
    let event_type =
        RuntimeEventType::from_value(&raw.event).ok_or(RuntimeImportReason::InvalidEnvelope)?;
    let skill_id = match (event_type.requires_skill(), raw.skill) {
        (true, SkillPresence::Present(Some(skill))) if safe_skill(&skill) => Some(skill),
        (false, SkillPresence::Absent) => None,
        _ => return Err(RuntimeImportReason::InvalidEnvelope),
    };
    let parsed_at = OffsetDateTime::parse(&raw.observed_at, &Rfc3339)
        .map_err(|_| RuntimeImportReason::InvalidTimestamp)?;
    if !raw.observed_at.ends_with('Z') || parsed_at.offset() != time::UtcOffset::UTC {
        return Err(RuntimeImportReason::InvalidTimestamp);
    }
    if parsed_at > now + MAX_FUTURE_SKEW {
        return Err(RuntimeImportReason::TimestampTooFarInFuture);
    }
    if parsed_at < now - MAX_EVENT_AGE {
        return Err(RuntimeImportReason::TimestampTooOld);
    }
    let observed_at = parsed_at
        .format(&Rfc3339)
        .map_err(|_| RuntimeImportReason::InvalidTimestamp)?;
    let content_hash = hash_scalars(
        &raw.event_id,
        agent_id,
        &raw.session_id,
        skill_id.as_deref(),
        event_type,
        &observed_at,
    );
    Ok(NormalizedEvent {
        event_id: raw.event_id,
        agent_id,
        session_id: raw.session_id,
        skill_id,
        event_type,
        observed_at,
        content_hash,
    })
}

fn classify_and_append(
    connection: &mut Connection,
    lines: Vec<LineResult>,
    now: OffsetDateTime,
) -> RuntimeImportSummary {
    let mut summary = RuntimeImportSummary::empty();
    let mut states = BTreeMap::new();
    let imported_at = match now.format(&Rfc3339) {
        Ok(value) => value,
        Err(_) => return RuntimeImportSummary::storage_unavailable(),
    };
    let transaction = match connection.transaction() {
        Ok(transaction) => transaction,
        Err(_) => return RuntimeImportSummary::storage_unavailable(),
    };
    for line in lines {
        match line {
            LineResult::Rejected(reason) => reject(&mut summary, reason),
            LineResult::Candidate(candidate) => {
                let existing_hash = match event_content_hash(&transaction, &candidate.event_id) {
                    Ok(hash) => hash,
                    Err(_) => {
                        let _ = transaction.rollback();
                        return RuntimeImportSummary::storage_unavailable();
                    }
                };
                if let Some(existing_hash) = existing_hash {
                    if existing_hash == candidate.content_hash {
                        summary.duplicate += 1;
                    } else {
                        reject(&mut summary, RuntimeImportReason::ConflictingEventId);
                    }
                    continue;
                }
                let key = (
                    candidate.agent_id.value().to_owned(),
                    candidate.session_id.clone(),
                );
                if !states.contains_key(&key) {
                    let stored = match session_lifecycle(
                        &transaction,
                        candidate.agent_id,
                        &candidate.session_id,
                    ) {
                        Ok(stored) => stored,
                        Err(_) => {
                            let _ = transaction.rollback();
                            return RuntimeImportSummary::storage_unavailable();
                        }
                    };
                    states.insert(
                        key.clone(),
                        Lifecycle {
                            started: stored.started,
                            ended: stored.ended,
                        },
                    );
                }
                let state = states.get(&key).copied().unwrap_or_default();
                let invalid = if state.ended {
                    Some(RuntimeImportReason::EventAfterSessionEnded)
                } else if state.started && candidate.event_type == RuntimeEventType::SessionStarted
                {
                    Some(RuntimeImportReason::DuplicateSessionStarted)
                } else if !state.started && candidate.event_type != RuntimeEventType::SessionStarted
                {
                    Some(RuntimeImportReason::MissingSessionStarted)
                } else {
                    None
                };
                if let Some(reason) = invalid {
                    reject(&mut summary, reason);
                    continue;
                }
                let event = RuntimeEvent {
                    event_id: candidate.event_id.clone(),
                    agent_id: candidate.agent_id,
                    session_id: candidate.session_id.clone(),
                    skill_id: candidate.skill_id,
                    event_type: candidate.event_type,
                    observed_at: candidate.observed_at,
                    imported_at: imported_at.clone(),
                    content_hash: candidate.content_hash,
                };
                match append_in_transaction(&transaction, &event) {
                    Ok(AppendResult::Inserted) => {
                        apply_lifecycle(&mut states, &key, event.event_type);
                        summary.accepted += 1;
                    }
                    Ok(AppendResult::Duplicate) => summary.duplicate += 1,
                    Err(AppendError::ConflictingEventId) => {
                        reject(&mut summary, RuntimeImportReason::ConflictingEventId)
                    }
                    Err(AppendError::InvalidEvent | AppendError::Database) => {
                        let _ = transaction.rollback();
                        return RuntimeImportSummary::storage_unavailable();
                    }
                }
            }
        }
    }
    if transaction.commit().is_err() {
        RuntimeImportSummary::storage_unavailable()
    } else {
        summary
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Lifecycle {
    started: bool,
    ended: bool,
}

fn apply_lifecycle(
    states: &mut BTreeMap<(String, String), Lifecycle>,
    key: &(String, String),
    event: RuntimeEventType,
) {
    let state = states.entry(key.clone()).or_default();
    match event {
        RuntimeEventType::SessionStarted => state.started = true,
        RuntimeEventType::SessionEnded => state.ended = true,
        _ => {}
    }
}

fn reject(summary: &mut RuntimeImportSummary, reason: RuntimeImportReason) {
    summary.rejected += 1;
    if !summary.reasons.contains(&reason) {
        summary.reasons.push(reason);
    }
}

fn hash_scalars(
    event_id: &str,
    agent: RuntimeAgent,
    session_id: &str,
    skill_id: Option<&str>,
    event: RuntimeEventType,
    observed_at: &str,
) -> String {
    let tuple = [
        "1",
        event_id,
        agent.value(),
        session_id,
        skill_id.unwrap_or(""),
        event.value(),
        observed_at,
    ];
    let mut digest = Sha256::new();
    for scalar in tuple {
        digest.update(scalar.as_bytes());
        digest.update([0]);
    }
    format!("{:x}", digest.finalize())
}

fn ensure_safe_directory(path: &Path) -> Result<(), RuntimeImportReason> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_link_or_reparse(&metadata) || !metadata.is_dir() => {
            Err(RuntimeImportReason::InboxPathRejected)
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(path).map_err(|_| RuntimeImportReason::InboxUnavailable)?;
            let metadata =
                fs::symlink_metadata(path).map_err(|_| RuntimeImportReason::InboxUnavailable)?;
            if is_link_or_reparse(&metadata) || !metadata.is_dir() {
                Err(RuntimeImportReason::InboxPathRejected)
            } else {
                Ok(())
            }
        }
        Err(_) => Err(RuntimeImportReason::InboxUnavailable),
    }
}

fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink() || is_reparse(metadata)
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse(_: &fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn rejects_unknown_fields_and_requires_session_start() {
        let root = tempfile::tempdir().unwrap();
        let inbox = RuntimeInbox::from_app_local_root(root.path());
        fs::create_dir_all(inbox.path().parent().unwrap()).unwrap();
        let now = OffsetDateTime::now_utc();
        let observed = now.format(&Rfc3339).unwrap();
        let mut file = File::create(inbox.path()).unwrap();
        writeln!(file, "{{\"schemaVersion\":1,\"eventId\":\"bad-extra\",\"agent\":\"codex\",\"sessionId\":\"s1\",\"event\":\"session.started\",\"observedAt\":\"{observed}\",\"payload\":\"no\"}}").unwrap();
        writeln!(file, "{{\"schemaVersion\":1,\"eventId\":\"no-start\",\"agent\":\"codex\",\"sessionId\":\"s2\",\"event\":\"skill.called\",\"skill\":\"code\",\"observedAt\":\"{observed}\"}}").unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        super::super::store::ensure_schema(&connection).unwrap();

        let result = import_inbox_at(&mut connection, &inbox, now);

        assert_eq!(result.accepted, 0);
        assert_eq!(result.rejected, 2);
        assert!(result
            .reasons
            .contains(&RuntimeImportReason::InvalidEnvelope));
        assert!(result
            .reasons
            .contains(&RuntimeImportReason::MissingSessionStarted));
    }

    #[test]
    fn imports_closed_lifecycle_idempotently() {
        let root = tempfile::tempdir().unwrap();
        let inbox = RuntimeInbox::from_app_local_root(root.path());
        fs::create_dir_all(inbox.path().parent().unwrap()).unwrap();
        let now = OffsetDateTime::now_utc();
        let observed = now.format(&Rfc3339).unwrap();
        let mut file = File::create(inbox.path()).unwrap();
        writeln!(file, "{{\"schemaVersion\":1,\"eventId\":\"start\",\"agent\":\"claude\",\"sessionId\":\"s1\",\"event\":\"session.started\",\"observedAt\":\"{observed}\"}}").unwrap();
        writeln!(file, "{{\"schemaVersion\":1,\"eventId\":\"load\",\"agent\":\"claude\",\"sessionId\":\"s1\",\"event\":\"skill.loaded\",\"skill\":\"research\",\"observedAt\":\"{observed}\"}}").unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        super::super::store::ensure_schema(&connection).unwrap();

        let first = import_inbox_at(&mut connection, &inbox, now);
        let second = import_inbox_at(&mut connection, &inbox, now);

        assert_eq!((first.accepted, first.rejected), (2, 0));
        assert_eq!((second.accepted, second.duplicate), (0, 2));
    }
}
