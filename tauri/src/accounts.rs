//! The accounts commands: signing up, in and out on this device, through
//! the web server, see `app_core::accounts`. The server is asked first;
//! its answer goes into the account's own file, see `device_accounts`,
//! which becomes the active one and syncs. A sign-out syncs first, so a
//! change still waiting reaches the server, then closes the account,
//! whose data stays for the next sign-in.

use tauri::{AppHandle, State};

use app_core::accounts::{self, Session};
use app_core::users::User;

use crate::device_accounts::DeviceAccounts;
use crate::sync::{self, SyncServer};

#[tauri::command]
pub async fn accounts_sign_up(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    email: String,
    password: String,
) -> Result<User, String> {
    let session = accounts::sign_up_through(server.transport(), &email, &password)
        .await
        .map_err(|error| error.to_string())?;
    keep_and_sync(app, &device, &server, session).await
}

#[tauri::command]
pub async fn accounts_sign_in(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    email: String,
    password: String,
) -> Result<User, String> {
    let session = accounts::sign_in_through(server.transport(), &email, &password)
        .await
        .map_err(|error| error.to_string())?;
    keep_and_sync(app, &device, &server, session).await
}

async fn keep_and_sync(
    app: AppHandle,
    device: &DeviceAccounts,
    server: &SyncServer,
    session: Session,
) -> Result<User, String> {
    let user = {
        let _no_sync_meanwhile = server.hold().await;
        device
            .keep_signed_in(session)
            .await
            .map_err(|error| error.to_string())?
    };
    sync::spawn(app);
    Ok(user)
}

#[tauri::command]
pub async fn accounts_sign_out(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
) -> Result<(), String> {
    // Reported by the sync itself; what it could not push waits in the
    // account's file for the next sign-in.
    let _ = sync::run(&app).await;
    let _no_sync_meanwhile = server.hold().await;
    device
        .sign_out(server.transport())
        .await
        .map_err(|error| error.to_string())
}
