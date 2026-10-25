//! Offline-first realms on the device, see `phoenix/docs/design/offline-sync.md`.
//!
//! The device keeps a small identity store, `identity.json` in the app's
//! data directory: the user and the realms they are connected to, with
//! their hosts, from the `identity.realms.list` request on the websocket
//! API. Each realm has its own SQLite file under `realms/<slug>.db` with
//! the same tables as the server, see `schema`. The pages read only from
//! that file, so they work with the network off.
//!
//! A local write, see `store`, upserts the row and queues it in
//! `sync_outbox`. A sync, see `worker`, pushes the outbox, pulls what
//! changed on the server, backfills a community the user just joined, and
//! prunes what the user can no longer see. One runs after every local
//! write, when a realm is opened, on demand, and every half minute.
//!
//! ```text
//! mod.rs     the identity store, the commands and the timer
//! schema.rs  opening a realm's file: the embedded client migrations
//! store.rs   reads and local writes on a file, applying server rows
//! http.rs    the three sync endpoints over HTTP with the API token
//! worker.rs  one sync of one realm
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

/// The connected user and their realm directory, as `identity.realms.list`
/// returns them.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Identity {
    pub user: Profile,
    pub realms: Vec<RealmInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub email: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RealmInfo {
    pub id: String,
    pub slug: String,
    pub kind: String,
    pub host: String,
}

/// A realm as a page shows it, read from the realm's file only.
#[derive(Debug, Serialize)]
pub struct RealmPage {
    pub realm: RealmInfo,
    pub communities: Vec<Community>,
    pub posts: Vec<FeedPost>,
    pub pending: i64,
    pub cursor: Option<Cursor>,
}

/// The identity store and the realm files on this device.
pub struct Device {
    dir: PathBuf,
    identity: Mutex<Option<Identity>>,
}

impl Device {
    pub fn open(data_dir: PathBuf) -> Self {
        let path = data_dir.join("identity.json");
        let identity = match fs::read_to_string(&path) {
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
            identity: Mutex::new(identity),
        }
    }

    pub fn identity(&self) -> Option<Identity> {
        self.identity
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn save_identity(&self, identity: &Identity) -> Result<(), String> {
        fs::create_dir_all(&self.dir).map_err(|error| error.to_string())?;
        let contents = serde_json::to_string_pretty(identity).map_err(|error| error.to_string())?;
        fs::write(self.dir.join("identity.json"), contents).map_err(|error| error.to_string())?;
        *self.identity.lock().unwrap_or_else(PoisonError::into_inner) = Some(identity.clone());
        Ok(())
    }

    pub fn realm(&self, slug: &str) -> Result<RealmInfo, String> {
        self.identity()
            .and_then(|identity| identity.realms.into_iter().find(|realm| realm.slug == slug))
            .ok_or_else(|| format!("This device knows no realm {slug}"))
    }

    pub fn realm_path(&self, slug: &str) -> PathBuf {
        self.dir.join("realms").join(format!("{slug}.db"))
    }

    /// Forgets the identity and every realm file, on log out.
    pub fn forget_all(&self) -> Result<(), String> {
        *self.identity.lock().unwrap_or_else(PoisonError::into_inner) = None;
        match fs::remove_file(self.dir.join("identity.json")) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        match fs::remove_dir_all(self.dir.join("realms")) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}

/// The device clock, in the wire format: UTC ISO 8601 with six fractional
/// digits, exactly as the server writes datetimes.
pub fn now_iso8601() -> String {
    chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.6fZ")
        .to_string()
}

/// The user's realm directory, refreshed from the server when connected,
/// else as last saved on this device.
#[tauri::command]
pub async fn list_realms(state: State<'_, AppState>) -> Result<Identity, String> {
    match crate::request(&state.control, "identity.realms.list".into(), json!({})).await {
        Ok(reply) => {
            let identity: Identity =
                serde_json::from_value(reply).map_err(|error| error.to_string())?;
            state.device.save_identity(&identity)?;
            Ok(identity)
        }
        Err(error) => state
            .device
            .identity()
            .ok_or_else(|| format!("Not connected, and this device knows no realms yet: {error}")),
    }
}

/// A realm's communities and feed, from its file on this device.
#[tauri::command]
pub fn get_realm_page(state: State<'_, AppState>, slug: String) -> Result<RealmPage, String> {
    page(&state.device, &slug)
}

/// Writes a post into the realm's file, in the communities given, and
/// starts a sync in the background. Returns the page as it is now.
#[tauri::command]
pub async fn write_post(
    app: AppHandle,
    state: State<'_, AppState>,
    slug: String,
    body: String,
    community_ids: Vec<String>,
) -> Result<RealmPage, String> {
    let device = &state.device;
    let identity = device.identity().ok_or("Not logged in on this device")?;
    let connection = schema::open(&device.realm_path(&slug))?;
    store::write_post(
        &connection,
        &identity.user.id,
        &body,
        &community_ids,
        &now_iso8601(),
    )?;
    drop(connection);

    spawn_sync(app, slug.clone());
    page(device, &slug)
}

/// Edits the user's own post in the realm's file and starts a sync.
#[tauri::command]
pub async fn edit_post(
    app: AppHandle,
    state: State<'_, AppState>,
    slug: String,
    post_id: String,
    body: String,
) -> Result<RealmPage, String> {
    let device = &state.device;
    let identity = device.identity().ok_or("Not logged in on this device")?;
    let connection = schema::open(&device.realm_path(&slug))?;
    store::edit_post(
        &connection,
        &identity.user.id,
        &post_id,
        &body,
        &now_iso8601(),
    )?;
    drop(connection);

    spawn_sync(app, slug.clone());
    page(device, &slug)
}

/// Syncs the realm now and returns what happened.
#[tauri::command]
pub async fn sync_realm(app: AppHandle, slug: String) -> Result<SyncReport, String> {
    run_sync(&app, &slug).await
}

fn page(device: &Device, slug: &str) -> Result<RealmPage, String> {
    let identity = device.identity().ok_or("Not logged in on this device")?;
    let realm = device.realm(slug)?;
    let connection = schema::open(&device.realm_path(slug))?;

    Ok(RealmPage {
        realm,
        communities: store::member_communities(&connection, &identity.user.id)?,
        posts: store::feed(&connection, &identity.user.id)?,
        pending: store::pending_count(&connection)?,
        cursor: store::cursor(&connection)?,
    })
}

/// One sync of one realm, reported to the main window as a `realm-synced`
/// event with the report or the error.
pub async fn run_sync(app: &AppHandle, slug: &str) -> Result<SyncReport, String> {
    let state = app.state::<AppState>();
    let device = &state.device;
    let identity = device.identity().ok_or("Not logged in on this device")?;
    let token = state.tokens.get().ok_or("Not logged in")?;
    let realm = device.realm(slug)?;
    let server = http::SyncServer::new(&state.config, &realm.host, &token)?;
    let mut connection = schema::open(&device.realm_path(slug))?;

    let result = worker::sync_realm(&server, &mut connection, &identity.user.id).await;

    if let Err(error) = &result {
        log::warn!("Sync of {slug} failed: {error}");
    }
    let event =
        json!({"slug": slug, "report": result.as_ref().ok(), "error": result.as_ref().err()});
    if let Err(error) = app.emit_to(MAIN_WINDOW, "realm-synced", event) {
        log::warn!("Could not emit the sync report: {error}");
    }
    result
}

fn spawn_sync(app: AppHandle, slug: String) {
    tauri::async_runtime::spawn(async move {
        let _ = run_sync(&app, &slug).await;
    });
}

/// Syncs every known realm every half minute while logged in.
pub async fn run_timer(app: AppHandle) {
    loop {
        tokio::time::sleep(SYNC_INTERVAL).await;

        let realms = {
            let state = app.state::<AppState>();
            if state.tokens.get().is_none() {
                continue;
            }
            state
                .device
                .identity()
                .map(|identity| identity.realms)
                .unwrap_or_default()
        };

        for realm in realms {
            let _ = run_sync(&app, &realm.slug).await;
        }
    }
}
