//! The user signed in on the active account, for every command that acts
//! as them, with the account's deployment, see `device_accounts`; an
//! error, "Nobody is signed in", until someone is active.

use tauri::State;

use app_core::deployment::Deployment;
use app_core::users::{self, User, UserError};

use crate::device_accounts::{DeviceAccounts, DeviceAccountsError};

pub async fn signed_in(device: &DeviceAccounts) -> Result<(Deployment, User), String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    let user = users::signed_in(&deployment)
        .await
        .map_err(|error| error.to_string())?;
    Ok((deployment, user))
}

/// The user signed in, or none when nobody is; an error only when the
/// device could not tell.
#[tauri::command]
pub async fn current_user_get(device: State<'_, DeviceAccounts>) -> Result<Option<User>, String> {
    let deployment = match device.active().await {
        Ok(deployment) => deployment,
        Err(DeviceAccountsError::NobodyActive) => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    match users::signed_in(&deployment).await {
        Ok(user) => Ok(Some(user)),
        Err(UserError::NotSignedIn) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}
