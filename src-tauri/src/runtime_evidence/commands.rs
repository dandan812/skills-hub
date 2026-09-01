use std::{path::PathBuf, sync::Mutex};

use rusqlite::Connection;
use tauri::State;

use crate::core::skill_store::SkillStore;

use super::{
    import::{import_inbox, RuntimeImportSummary, RuntimeInbox},
    projection::{project_overview, CatalogSnapshot, RuntimeOverview},
    store::{ensure_schema, list_events, RuntimeEvent},
    RuntimeEvidenceState,
};

#[tauri::command]
pub async fn get_runtime_evidence_overview(
    runtime: State<'_, RuntimeEvidenceState>,
    store: State<'_, SkillStore>,
) -> Result<RuntimeOverview, String> {
    let db_path = runtime.db_path.clone();
    let inbox_path = runtime.inbox.as_ref().map(RuntimeInbox::path);
    let last_import = runtime.last_import.clone();
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        build_overview(db_path, inbox_path, &last_import, &store)
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn refresh_runtime_evidence(
    runtime: State<'_, RuntimeEvidenceState>,
    store: State<'_, SkillStore>,
) -> Result<RuntimeOverview, String> {
    let db_path = runtime.db_path.clone();
    let inbox = runtime.inbox.clone();
    let last_import = runtime.last_import.clone();
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let summary = match (&db_path, &inbox) {
            (Some(db_path), Some(inbox)) => match Connection::open(db_path) {
                Ok(mut connection) if ensure_schema(&connection).is_ok() => {
                    import_inbox(&mut connection, inbox)
                }
                _ => RuntimeImportSummary::storage_unavailable(),
            },
            _ => RuntimeImportSummary::storage_unavailable(),
        };
        if let Ok(mut guard) = last_import.lock() {
            *guard = summary;
        }
        build_overview(
            db_path,
            inbox.as_ref().map(RuntimeInbox::path),
            &last_import,
            &store,
        )
    })
    .await
    .map_err(|error| error.to_string())
}

fn build_overview(
    db_path: Option<PathBuf>,
    inbox_path: Option<PathBuf>,
    last_import: &Mutex<RuntimeImportSummary>,
    store: &SkillStore,
) -> RuntimeOverview {
    let catalog = CatalogSnapshot::load(store).unwrap_or_else(|_| CatalogSnapshot::unavailable());
    let (events, storage_failed) = read_events(db_path.as_ref());
    let import = if storage_failed {
        RuntimeImportSummary::storage_unavailable()
    } else {
        last_import
            .lock()
            .map(|summary| summary.clone())
            .unwrap_or_else(|_| RuntimeImportSummary::storage_unavailable())
    };
    project_overview(
        &catalog,
        &events,
        import,
        inbox_path.map(|path| path.to_string_lossy().into_owned()),
    )
}

fn read_events(db_path: Option<&PathBuf>) -> (Vec<RuntimeEvent>, bool) {
    let Some(db_path) = db_path else {
        return (Vec::new(), true);
    };
    match Connection::open(db_path).and_then(|connection| {
        ensure_schema(&connection)?;
        list_events(&connection)
    }) {
        Ok(events) => (events, false),
        Err(_) => (Vec::new(), true),
    }
}
