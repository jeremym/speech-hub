//! User-defined Sources. Milestone 4 fills this in; the IPC layer already routes here.

use speechhub_protocol::ErrorCode;
use tauri::AppHandle;

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("no Source named '{0}'")]
    NotFound(String),
    #[error("Source '{0}' has not been approved to run yet; open Preferences → Sources")]
    NotTrusted(String),
    #[error("Source failed: {0}")]
    Failed(String),
    #[error("{0}")]
    Hub(#[from] crate::hub::HubError),
}

impl SourceError {
    pub fn code(&self) -> ErrorCode {
        match self {
            SourceError::NotFound(_) => ErrorCode::SourceNotFound,
            SourceError::NotTrusted(_) => ErrorCode::SourceNotTrusted,
            SourceError::Failed(_) => ErrorCode::SourceFailed,
            SourceError::Hub(e) => e.code(),
        }
    }
}

/// Run a Source by id or (case-insensitive) name. Public entry point for CLI, palette, hotkey.
pub fn run_by_ref(_app: &AppHandle, reference: &str) -> Result<serde_json::Value, SourceError> {
    Err(SourceError::NotFound(reference.to_string()))
}

/// Public listing for `speak sources list` (no secrets: ids, names, types only).
pub fn list_public(_app: &AppHandle) -> Vec<serde_json::Value> {
    Vec::new()
}

/// Deep links may *ask* to run a Source; the user must confirm in the UI (M4).
pub fn confirm_and_run_from_link(app: &AppHandle, reference: &str) {
    crate::hub::get(app).notice(format!("A link asked to run Source “{reference}”. Running Sources from links arrives in Milestone 4."));
}
