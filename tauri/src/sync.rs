//! Syncing the device's database with the web server, see
//! `docs/design/offline-sync.md`: `app_core::sync::run` over HTTP to the
//! server at `SYNC_SERVER_URL`, read when the app starts, else the value
//! compiled in, which is how a phone build learns it, else the dev
//! server's. One sync runs at a time: when the app starts, when it comes
//! to the foreground, after every write, from the Sync now button, and
//! every half minute. Each one ends with a `synced` event to the pages,
//! carrying the report or the error.
//! A device working offline, `sync_set_offline`, makes no request until
//! it goes online again, and neither does one nobody is signed in on.
//! A sync is a spawned task, not a queued job, and the only queue is the
//! pending events: see "How a sync is started and run" in the design.

use std::env;
use std::sync::{Mutex as SyncMutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::{App, AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

use app_core::deployment::Deployment;
use app_core::sync::{self, Http, Report, Status};

const DEFAULT_SERVER_URL: &str = "http://localhost:4000";
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
}

pub fn setup(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let url = server_url();
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

fn server_url() -> String {
    env::var("SYNC_SERVER_URL")
        .ok()
        .or_else(|| option_env!("SYNC_SERVER_URL").map(String::from))
        .unwrap_or_else(|| DEFAULT_SERVER_URL.to_string())
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

/// One sync, reported to the pages as a `synced` event.
pub async fn run(app: &AppHandle) -> Result<Report, String> {
    let server = app.state::<SyncServer>();
    let deployment = app.state::<Deployment>();
    let _one_at_a_time = server.one_at_a_time.lock().await;
    *server
        .last_started
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());
    let result = sync::run(&deployment, &server.transport)
        .await
        .map_err(|error| error.to_string());
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
    deployment: State<'_, Deployment>,
    offline: bool,
) -> Result<(), String> {
    sync::set_offline(&deployment, offline)
        .await
        .map_err(|error| error.to_string())?;
    if !offline {
        spawn(app);
    }
    Ok(())
}

#[tauri::command]
pub async fn sync_status(deployment: State<'_, Deployment>) -> Result<Status, String> {
    sync::status(&deployment)
        .await
        .map_err(|error| error.to_string())
}
