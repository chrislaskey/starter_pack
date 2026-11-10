//! The files commands: `app_core::files` as the pages call it with
//! `invoke`, for the user signed in on this device, see
//! `docs/design/offline-files-decisions.md`. A form's error is
//! `{error, errors}`, the web's 422 body. A write starts a sync in the
//! background once it is on the device, see `sync`.

use tauri::{AppHandle, State};
use uuid::Uuid;

use app_core::files::{self, File, FileError, FileWithState};

use crate::avatars::FormError;
use crate::device_accounts::DeviceAccounts;
use crate::sync;
use crate::users::signed_in;

impl From<FileError> for FormError {
    fn from(error: FileError) -> FormError {
        match error {
            FileError::Invalid(errors) => FormError::of_fields(errors),
            other => FormError::from(other.to_string()),
        }
    }
}

#[tauri::command]
pub async fn files_list(device: State<'_, DeviceAccounts>) -> Result<Vec<FileWithState>, String> {
    let (deployment, user) = signed_in(&device).await?;
    files::list(&deployment, &user)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn files_get(
    device: State<'_, DeviceAccounts>,
    id: Uuid,
) -> Result<FileWithState, String> {
    let (deployment, user) = signed_in(&device).await?;
    files::get(&deployment, &user, id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn files_update(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    id: Uuid,
    name: String,
) -> Result<File, FormError> {
    let (deployment, user) = signed_in(&device).await?;
    let file = files::update(&deployment, &user, id, &name).await?;
    sync::spawn(app);
    Ok(file)
}

#[tauri::command]
pub async fn files_delete(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    id: Uuid,
) -> Result<(), String> {
    let (deployment, user) = signed_in(&device).await?;
    files::delete(&deployment, &user, id)
        .await
        .map_err(|error| error.to_string())?;
    sync::spawn(app);
    Ok(())
}
