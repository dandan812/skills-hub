pub mod commands;
mod import;
mod projection;
mod store;

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use import::{RuntimeImportSummary, RuntimeInbox};
use rusqlite::Connection;

pub const RUNTIME_EVIDENCE_SCHEMA_VERSION: u8 = 1;

pub struct RuntimeEvidenceState {
    db_path: Option<PathBuf>,
    inbox: Option<RuntimeInbox>,
    last_import: Arc<Mutex<RuntimeImportSummary>>,
}

impl RuntimeEvidenceState {
    pub fn from_app_local_root(app_local_root: &Path) -> Self {
        let db_path = app_local_root.join("runtime-evidence.sqlite3");
        let available = std::fs::create_dir_all(app_local_root)
            .and_then(|()| {
                Connection::open(&db_path)
                    .map_err(std::io::Error::other)
                    .and_then(|connection| {
                        store::ensure_schema(&connection).map_err(std::io::Error::other)
                    })
            })
            .is_ok();

        if available {
            Self {
                db_path: Some(db_path),
                inbox: Some(RuntimeInbox::from_app_local_root(app_local_root)),
                last_import: Arc::new(Mutex::new(RuntimeImportSummary::empty())),
            }
        } else {
            Self::unavailable()
        }
    }

    pub fn unavailable() -> Self {
        Self {
            db_path: None,
            inbox: None,
            last_import: Arc::new(Mutex::new(RuntimeImportSummary::storage_unavailable())),
        }
    }
}
