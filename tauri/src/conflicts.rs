//! The conflicts commands: `app_core::conflicts` as the conflicts page
//! calls it. A change reapplied starts a sync, see `sync`.

use tauri::{AppHandle, State};
use uuid::Uuid;

use app_core::conflicts::{self, Conflict};
use app_core::deployment::Deployment;

use crate::sync;

#[tauri::command]
pub async fn conflicts_list(deployment: State<'_, Deployment>) -> Result<Vec<Conflict>, String> {
    conflicts::list(&deployment)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn conflicts_reapply(
    app: AppHandle,
    deployment: State<'_, Deployment>,
    id: Uuid,
) -> Result<(), String> {
    conflicts::reapply(&deployment, id)
        .await
        .map_err(|error| error.to_string())?;
    sync::spawn(app);
    Ok(())
}

#[tauri::command]
pub async fn conflicts_let_go(deployment: State<'_, Deployment>, id: Uuid) -> Result<(), String> {
    conflicts::let_go(&deployment, id)
        .await
        .map_err(|error| error.to_string())
}
