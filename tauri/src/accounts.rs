//! The accounts commands: signing in and out on this device. A sign-in
//! happens in the system browser, on the web's own sign-in page, see
//! `docs/design/accounts-sign-in.md`: `accounts_sign_in_start` makes
//! the verifier, keeps it in `device.db`, and opens the browser at
//! `BROWSER_SIGN_IN_URL` with the verifier's challenge; the page, done,
//! shows a code the person pastes into the app, and
//! `accounts_sign_in_finish` exchanges it with the verifier for the
//! session, which goes into the account's own file, see
//! `device_accounts`, which becomes the active one and syncs. The app
//! never sees a password. A sign-out syncs first, so a change still
//! waiting reaches the server, then closes the account, whose data stays
//! for the next sign-in.

use std::env;

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use app_core::accounts;
use app_core::users::User;

use crate::device_accounts::{DeviceAccounts, PendingSignIn};
use crate::sync::{self, SyncServer};

/// Keeps a verifier and opens the browser on the sign-in page, with the
/// email filled in for an account signed out on this device.
#[tauri::command]
pub async fn accounts_sign_in_start(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    email: Option<String>,
) -> Result<(), String> {
    let (verifier, challenge) = accounts::verifier_and_challenge();
    device
        .sign_in_start(&verifier, email.as_deref())
        .await
        .map_err(|error| error.to_string())?;
    let url = browser_sign_in_url(&challenge, email.as_deref());
    log::info!("opening the browser to sign in at {url}");
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| format!("could not open the browser: {error}"))
}

/// The sign-in waiting on the browser, for the accounts page to show
/// its dialog again after a relaunch.
#[tauri::command]
pub async fn accounts_sign_in_pending(
    device: State<'_, DeviceAccounts>,
) -> Result<Option<PendingSignIn>, String> {
    device
        .sign_in_pending()
        .await
        .map_err(|error| error.to_string())
}

/// Exchanges the code the browser showed, with the verifier kept at the
/// start, for the session, and keeps it as the active account.
#[tauri::command]
pub async fn accounts_sign_in_finish(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    code: String,
) -> Result<User, String> {
    let verifier = device
        .sign_in_verifier()
        .await
        .map_err(|error| error.to_string())?
        .ok_or("No sign-in is waiting; start again")?;
    let session =
        accounts::sign_in_with_device_code_through(server.transport(), code.trim(), &verifier)
            .await
            .map_err(|error| error.to_string())?;
    let user = {
        let _no_sync_meanwhile = server.hold().await;
        device
            .keep_signed_in(session)
            .await
            .map_err(|error| error.to_string())?
    };
    device
        .sign_in_clear()
        .await
        .map_err(|error| error.to_string())?;
    sync::spawn(app);
    Ok(user)
}

#[tauri::command]
pub async fn accounts_sign_in_cancel(device: State<'_, DeviceAccounts>) -> Result<(), String> {
    device
        .sign_in_clear()
        .await
        .map_err(|error| error.to_string())
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

/// The web's sign-in page, with the challenge and the email: at
/// `BROWSER_SIGN_IN_URL`, read when the app starts, else the value
/// compiled in, else the sync server's own `/accounts/sign-in`, which
/// is where the pages are in production.
fn browser_sign_in_url(challenge: &str, email: Option<&str>) -> String {
    let page = env::var("BROWSER_SIGN_IN_URL")
        .ok()
        .or_else(|| option_env!("BROWSER_SIGN_IN_URL").map(String::from))
        .unwrap_or_else(|| format!("{}/accounts/sign-in", sync::server_url()));
    let mut url = format!("{page}?challenge={}", encode(challenge));
    if let Some(email) = email.filter(|email| !email.is_empty()) {
        url.push_str(&format!("&email={}", encode(email)));
    }
    url
}

/// Percent-encodes a query value: everything but the unreserved
/// characters.
fn encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accounts_browser_sign_in_url_carries_the_challenge_and_the_email_encoded() {
        let url = browser_sign_in_url("abc123", Some("alice+1@example.com"));
        assert!(
            url.ends_with("/accounts/sign-in?challenge=abc123&email=alice%2B1%40example.com"),
            "{url}"
        );
        let url = browser_sign_in_url("abc123", Some(""));
        assert!(url.ends_with("?challenge=abc123"), "{url}");
    }
}
