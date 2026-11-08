//! The pending changes command: `app_core::pending_changes` as the
//! page listing the changes not on the server calls it.

use tauri::State;

use app_core::deployment::Deployment;
use app_core::pending_changes::{self, PendingChange};

#[tauri::command]
pub async fn pending_changes_list(
    deployment: State<'_, Deployment>,
) -> Result<Vec<PendingChange>, String> {
    pending_changes::list(&deployment)
        .await
        .map_err(|error| error.to_string())
}
