//! The user signed in on the active account, for every command that acts
//! as them, with the account's deployment, see `device_accounts`; an
//! error, "Nobody is signed in", until someone is active.

use serde::Serialize;
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

/// The user signed in as their own pages show them: with the email of
/// their account, which the device keeps in its list of accounts and
/// their synced row does not carry.
#[derive(Serialize)]
pub struct CurrentUserShown {
    #[serde(flatten)]
    pub user: User,
    pub email: String,
}

/// The user signed in, or none when nobody is; an error only when the
/// device could not tell.
#[tauri::command]
pub async fn current_user_get(
    device: State<'_, DeviceAccounts>,
) -> Result<Option<CurrentUserShown>, String> {
    let deployment = match device.active().await {
        Ok(deployment) => deployment,
        Err(DeviceAccountsError::NobodyActive) => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let user = match users::signed_in(&deployment).await {
        Ok(user) => user,
        Err(UserError::NotSignedIn) => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let accounts = device.list().await.map_err(|error| error.to_string())?;
    let email = accounts
        .into_iter()
        .find(|account| account.user_id == user.id)
        .map(|account| account.email)
        .ok_or("The signed-in account is not in this device's list")?;
    Ok(Some(CurrentUserShown { user, email }))
}
