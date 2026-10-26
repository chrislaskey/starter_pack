//! Offline-first communities on the device, see
//! `phoenix/docs/design/offline-sync.md`.
//!
//! The device keeps the user it is logged in as in `profile.json` in the
//! app's data directory, from the `auth.user.get` request on the
//! websocket API, and the synced tables in one SQLite file, `app.db`, with
//! the same tables as the server, see `schema`. The pages read only from
//! that file, so they work with the network off.
//!
//! A local write, see `store`, upserts the row and queues it in
//! `sync_outbox`. A sync, see `worker`, pushes the outbox, pulls what
//! changed on the server, backfills a community the user just joined, and
//! prunes what the user can no longer see. One runs after every local
//! write, when the page is opened, on demand, and every half minute.
//!
//! ```text
//! mod.rs     the profile, the commands and the timer
//! schema.rs  opening the file: the embedded client migrations
//! store.rs   reads and local writes on the file, applying server rows
//! http.rs    the three sync endpoints over HTTP with the API token
//! worker.rs  one sync
//! ```

pub mod http;
pub mod schema;
pub mod store;
pub mod worker;

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{AppState, MAIN_WINDOW};
use store::{Community, Cursor, FeedPost};
use worker::SyncReport;

const SYNC_INTERVAL: Duration = Duration::from_secs(30);

/// The user this device is logged in as, as `auth.user.get` returns them.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub email: String,
}

/// The communities page, read from the device's file only.
#[derive(Debug, Serialize)]
pub struct CommunitiesPage {
    pub communities: Vec<Community>,
    pub posts: Vec<FeedPost>,
    pub pending: i64,
    pub cursor: Option<Cursor>,
}

/// The profile and the file on this device.
pub struct Device {
    dir: PathBuf,
    profile: Mutex<Option<Profile>>,
    /// Held during a sync, so two syncs never overlap.
    sync_lock: tokio::sync::Mutex<()>,
}

impl Device {
    pub fn open(data_dir: PathBuf) -> Self {
        let path = data_dir.join("profile.json");
        let profile = match fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents)
                .map_err(|error| log::warn!("Ignoring {}: {error}", path.display()))
                .ok(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => {
                log::warn!("Could not read {}: {error}", path.display());
                None
            }
        };

        Self {
            dir: data_dir,
            profile: Mutex::new(profile),
            sync_lock: tokio::sync::Mutex::new(()),
        }
    }

    pub fn profile(&self) -> Option<Profile> {
        self.profile
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn save_profile(&self, profile: &Profile) -> Result<(), String> {
        fs::create_dir_all(&self.dir).map_err(|error| error.to_string())?;
        let contents = serde_json::to_string_pretty(profile).map_err(|error| error.to_string())?;
        fs::write(self.dir.join("profile.json"), contents).map_err(|error| error.to_string())?;
        *self.profile.lock().unwrap_or_else(PoisonError::into_inner) = Some(profile.clone());
        Ok(())
    }

    /// The synced tables' file.
    pub fn db_path(&self) -> PathBuf {
        self.dir.join("app.db")
    }

    /// Forgets the profile and the file, on log out.
    pub fn forget_all(&self) -> Result<(), String> {
        *self.profile.lock().unwrap_or_else(PoisonError::into_inner) = None;
        for path in [self.dir.join("profile.json"), self.db_path()] {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        Ok(())
    }
}

/// The device clock, in the wire format: UTC ISO 8601 with six fractional
/// digits, exactly as the server writes datetimes.
pub fn now_iso8601() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.6fZ")
        .to_string()
}

/// The user this device is logged in as, refreshed from the server when
/// connected, else as last saved on this device.
#[tauri::command]
pub async fn get_profile(state: State<'_, AppState>) -> Result<Profile, String> {
    match crate::request(&state.control, "auth.user.get".into(), json!({})).await {
        Ok(reply) => {
            let profile: Profile =
                serde_json::from_value(reply).map_err(|error| error.to_string())?;
            state.device.save_profile(&profile)?;
            Ok(profile)
        }
        Err(error) => state
            .device
            .profile()
            .ok_or_else(|| format!("Not connected, and this device knows no user yet: {error}")),
    }
}

/// The user's communities and feed, from the file on this device.
#[tauri::command]
pub fn get_communities_page(state: State<'_, AppState>) -> Result<CommunitiesPage, String> {
    page(&state.device)
}

/// Writes a post into the file, in the communities given, and starts a
/// sync in the background. Returns the page as it is now.
#[tauri::command]
pub async fn write_post(
    app: AppHandle,
    state: State<'_, AppState>,
    body: String,
    community_ids: Vec<String>,
) -> Result<CommunitiesPage, String> {
    let device = &state.device;
    let profile = device.profile().ok_or("Not logged in on this device")?;
    let connection = schema::open(&device.db_path())?;
    store::write_post(
        &connection,
        &profile.id,
        &body,
        &community_ids,
        &now_iso8601(),
    )?;
    drop(connection);

    spawn_sync(app);
    page(device)
}

/// Edits the user's own post in the file and starts a sync.
#[tauri::command]
pub async fn edit_post(
    app: AppHandle,
    state: State<'_, AppState>,
    post_id: String,
    body: String,
) -> Result<CommunitiesPage, String> {
    let device = &state.device;
    let profile = device.profile().ok_or("Not logged in on this device")?;
    let connection = schema::open(&device.db_path())?;
    store::edit_post(&connection, &profile.id, &post_id, &body, &now_iso8601())?;
    drop(connection);

    spawn_sync(app);
    page(device)
}

/// Syncs now and returns what happened.
#[tauri::command]
pub async fn sync_now(app: AppHandle) -> Result<SyncReport, String> {
    run_sync(&app).await
}

fn page(device: &Device) -> Result<CommunitiesPage, String> {
    let profile = device.profile().ok_or("Not logged in on this device")?;
    let connection = schema::open(&device.db_path())?;

    Ok(CommunitiesPage {
        communities: store::member_communities(&connection, &profile.id)?,
        posts: store::feed(&connection, &profile.id)?,
        pending: store::pending_count(&connection)?,
        cursor: store::cursor(&connection)?,
    })
}

/// One sync, reported to the main window as a `synced` event with the
/// report or the error.
pub async fn run_sync(app: &AppHandle) -> Result<SyncReport, String> {
    let state = app.state::<AppState>();
    let device = &state.device;
    let profile = device.profile().ok_or("Not logged in on this device")?;
    let token = state.tokens.get().ok_or("Not logged in")?;
    let server = http::SyncServer::new(&state.config, &token)?;
    let _one_sync_at_a_time = device.sync_lock.lock().await;
    let mut connection = schema::open(&device.db_path())?;

    let result = worker::sync(&server, &mut connection, &profile.id).await;

    if let Err(error) = &result {
        log::warn!("Sync failed: {error}");
    }
    let event = json!({"report": result.as_ref().ok(), "error": result.as_ref().err()});
    if let Err(error) = app.emit_to(MAIN_WINDOW, "synced", event) {
        log::warn!("Could not emit the sync report: {error}");
    }
    result
}

fn spawn_sync(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let _ = run_sync(&app).await;
    });
}

/// Syncs every half minute while logged in, once the device knows its user.
pub async fn run_timer(app: AppHandle) {
    loop {
        tokio::time::sleep(SYNC_INTERVAL).await;

        let ready = {
            let state = app.state::<AppState>();
            state.tokens.get().is_some() && state.device.profile().is_some()
        };
        if ready {
            let _ = run_sync(&app).await;
        }
    }
}
