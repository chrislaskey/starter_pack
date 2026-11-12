//! Syncing the active account's database with the web server, see
//! `docs/design/offline-sync.md`: `app_core::sync::run` over HTTP to the
//! server at the URL `server_urls` resolves when the app starts. One
//! sync runs at a time: when the app starts, when it comes
//! to the foreground, after every write, from the Sync now button, and
//! every half minute. Each one ends with a `synced` event to the pages,
//! carrying the report or the error. A sync that went through also asks
//! for the person's plan when the one kept is stale, see
//! `app_core::accounts::online`.
//! A device working offline, `sync_set_offline`, makes no request until
//! it goes online again, and neither does one with no account active,
//! see `device_accounts`, which holds the same lock while it switches
//! files. A 401 means the server has forgotten the token: the account is
//! signed out on the device, its data kept.
//! A sync is a spawned task, not a queued job, and the only queue is the
//! pending events: see "How a sync is started and run" in the design.

use std::sync::{Mutex as SyncMutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::{App, AppHandle, Emitter, Manager, State};
use tokio::sync::{Mutex, MutexGuard};

use app_core::accounts::online;
use app_core::sync::{self, Http, Report, Status, SyncError};

use crate::device_accounts::DeviceAccounts;
use crate::server_urls;

const INTERVAL: Duration = Duration::from_secs(30);
/// A foreground sync is skipped this soon after the last sync started: a
/// resume and a focus arrive together, and a click back into a desktop
/// window is not news.
const FOREGROUND_GAP: Duration = Duration::from_secs(5);

/// The server, the lock one sync at a time holds, and when the last
/// sync started.
pub struct SyncServer {
    transport: Http,
    one_at_a_time: Mutex<()>,
    last_started: SyncMutex<Option<Instant>>,
}

impl SyncServer {
    /// The server, for the accounts commands to sign up, in and out through.
    pub fn transport(&self) -> &Http {
        &self.transport
    }

    /// Keeps any sync from starting, and waits for one in flight to
    /// finish, while the guard is held: for the moments the active
    /// account's file changes under the commands.
    pub async fn hold(&self) -> MutexGuard<'_, ()> {
        self.one_at_a_time.lock().await
    }
}

pub fn setup(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let url = server_urls::sync_server_url(app.handle());
    log::info!("sync server {url}");
    app.manage(SyncServer {
        transport: Http::new(&url)?,
        one_at_a_time: Mutex::new(()),
        last_started: SyncMutex::new(None),
    });
    spawn(app.handle().clone());
    let handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(INTERVAL).await;
            // Logged and reported to the pages by `run` itself
            let _ = run(&handle).await;
        }
    });
    Ok(())
}

/// Starts a sync in the background.
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let _ = run(&app).await;
    });
}

/// Starts a sync because the app came to the foreground: the window
/// gained focus, or the app resumed on a phone. Skipped when a sync
/// started within `FOREGROUND_GAP`.
pub fn spawn_on_foreground(app: &AppHandle) {
    let recent = app
        .state::<SyncServer>()
        .last_started
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_some_and(|started| started.elapsed() < FOREGROUND_GAP);
    if !recent {
        spawn(app.clone());
    }
}

/// One sync of the active account, reported to the pages as a `synced`
/// event; with nobody active, a report saying so.
pub async fn run(app: &AppHandle) -> Result<Report, String> {
    let server = app.state::<SyncServer>();
    let device = app.state::<DeviceAccounts>();
    let _one_at_a_time = server.one_at_a_time.lock().await;
    *server
        .last_started
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());
    let result = match device.active().await {
        Ok(deployment) => sync::run(&deployment, &server.transport).await,
        Err(_) => Ok(Report {
            signed_out: true,
            ..Report::default()
        }),
    };
    if let Err(SyncError::Unauthorized) = &result {
        log::warn!("the server no longer knows the token: signing out, data kept");
        if let Err(error) = device.forget_signed_in().await {
            log::warn!("could not sign out: {error}");
        }
    }
    if let (Ok(report), Ok(deployment)) = (&result, device.active().await)
        && !report.signed_out
        && !report.local
        && !report.offline
        && online::plan_is_stale(&deployment).await.unwrap_or(true)
    {
        crate::purchasing::plan_refresh(&deployment, &server.transport).await;
    }
    let result = result.map_err(|error| error.to_string());
    match &result {
        Ok(report) => log::info!("synced: {report:?}"),
        Err(error) => log::warn!("sync failed: {error:?}"),
    }
    let event = json!({ "report": result.as_ref().ok(), "error": result.as_ref().err() });
    if let Err(error) = app.emit("synced", event) {
        log::warn!("could not report the sync: {error}");
    }
    result
}

#[tauri::command]
pub async fn sync_run(app: AppHandle) -> Result<Report, String> {
    run(&app).await
}

/// Works offline, or online again, which syncs at once.
#[tauri::command]
pub async fn sync_set_offline(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    offline: bool,
) -> Result<(), String> {
    let deployment = device.active().await.map_err(|error| error.to_string())?;
    sync::set_offline(&deployment, offline)
        .await
        .map_err(|error| error.to_string())?;
    if !offline {
        spawn(app);
    }
    Ok(())
}

#[tauri::command]
pub async fn sync_status(device: State<'_, DeviceAccounts>) -> Result<Status, String> {
    let deployment = match device.active().await {
        Ok(deployment) => deployment,
        Err(_) => return Ok(Status::default()),
    };
    sync::status(&deployment)
        .await
        .map_err(|error| error.to_string())
}
