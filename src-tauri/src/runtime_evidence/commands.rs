use super::{current_status, RuntimeEvidenceStatus};

#[tauri::command]
pub fn get_runtime_evidence_status() -> RuntimeEvidenceStatus {
    current_status()
}
