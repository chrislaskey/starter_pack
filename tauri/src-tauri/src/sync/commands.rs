//! The commands the main window's pages call. A page reads its own
//! content, see `pages`; a write changes the file, starts a sync in the
//! background and returns, and the page reads itself again.

use rusqlite::Connection;
use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, State};

use super::pages::{
    self, CommunitiesPage, CommunityPage, PostsPage, ProfilePage, SyncStatus, TagsPage,
};
use super::worker::SyncReport;
use super::{Device, Profile, commenting, memberships, now_iso8601, posting, schema, store};
use super::{run_sync, spawn_sync, tagging};
use crate::{AppState, forms};
use dynamic_form::{FormState, Shown};

/// The file and the user's id, for a read.
fn open(device: &Device) -> Result<(Connection, String), String> {
    let profile = device.profile().ok_or("Not logged in on this device")?;
    Ok((schema::open(&device.db_path())?, profile.id))
}

/// Runs a write on the file as the user, at the device's time now, then
/// starts a sync.
fn write<T>(
    app: AppHandle,
    state: &State<'_, AppState>,
    change: impl FnOnce(&Connection, &str, &str) -> Result<T, String>,
) -> Result<T, String> {
    let (connection, user_id) = open(&state.device)?;
    let written = change(&connection, &user_id, &now_iso8601())?;
    drop(connection);
    spawn_sync(app);
    Ok(written)
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

// Sync

/// The changes waiting to be pushed and the conflicts, for every page.
#[tauri::command]
pub fn get_sync_status(state: State<'_, AppState>) -> Result<SyncStatus, String> {
    let (connection, _user_id) = open(&state.device)?;
    pages::sync_status(&connection)
}

/// Syncs now and returns what happened.
#[tauri::command]
pub async fn sync_now(app: AppHandle) -> Result<SyncReport, String> {
    run_sync(&app).await
}

/// `reapply` queues the change again and starts a sync, `ignore` lets it go.
#[tauri::command]
pub async fn resolve_conflict(
    app: AppHandle,
    state: State<'_, AppState>,
    event_id: String,
    resolution: String,
) -> Result<(), String> {
    let reapply = match resolution.as_str() {
        "reapply" => true,
        "ignore" => false,
        other => return Err(format!("Unknown resolution {other}")),
    };
    write(app, &state, |connection, user_id, now| {
        store::resolve_conflict(connection, user_id, &event_id, reapply, now)
    })
}

// Communities

#[tauri::command]
pub fn get_communities_page(state: State<'_, AppState>) -> Result<CommunitiesPage, String> {
    let (connection, user_id) = open(&state.device)?;
    pages::communities_page(&connection, &user_id)
}

/// The community's page, or nothing when the user is not a member of it.
#[tauri::command]
pub fn get_community_page(
    state: State<'_, AppState>,
    community_id: String,
) -> Result<Option<CommunityPage>, String> {
    let (connection, user_id) = open(&state.device)?;
    pages::community_page(&connection, &user_id, &community_id)
}

#[tauri::command]
pub async fn leave_community(
    app: AppHandle,
    state: State<'_, AppState>,
    community_id: String,
) -> Result<(), String> {
    write(app, &state, |connection, user_id, now| {
        memberships::leave(connection, user_id, &community_id, now)
    })
}

#[tauri::command]
pub async fn remove_post_from_community(
    app: AppHandle,
    state: State<'_, AppState>,
    post_id: String,
    community_id: String,
) -> Result<(), String> {
    write(app, &state, |connection, user_id, now| {
        posting::remove_from_community(connection, user_id, &post_id, &community_id, now)
    })
}

#[tauri::command]
pub async fn create_comment(
    app: AppHandle,
    state: State<'_, AppState>,
    posts_communities_id: String,
    body: String,
) -> Result<(), String> {
    write(app, &state, |connection, user_id, now| {
        commenting::comment(connection, user_id, &posts_communities_id, &body, now).map(drop)
    })
}

#[tauri::command]
pub async fn delete_comment(
    app: AppHandle,
    state: State<'_, AppState>,
    comment_id: String,
) -> Result<(), String> {
    write(app, &state, |connection, user_id, now| {
        commenting::delete_comment(connection, user_id, &comment_id, now)
    })
}

// Posts

#[tauri::command]
pub fn get_posts_page(state: State<'_, AppState>) -> Result<PostsPage, String> {
    let (connection, user_id) = open(&state.device)?;
    pages::posts_page(&connection, &user_id)
}

fn member_communities(state: &State<'_, AppState>) -> Result<Vec<store::Community>, String> {
    let (connection, user_id) = open(&state.device)?;
    store::member_communities(&connection, &user_id)
}

/// The empty post form, as it first shows.
#[tauri::command]
pub fn show_post_form(state: State<'_, AppState>) -> Result<Shown, String> {
    forms::show_post_form(&member_communities(&state)?)
}

/// The post form after a change: its `FormData` entries, and the state it
/// last showed with.
#[tauri::command]
pub fn change_post_form(
    state: State<'_, AppState>,
    form: FormState,
    entries: Vec<(String, String)>,
) -> Result<Shown, String> {
    forms::change_post_form(&member_communities(&state)?, &form, &entries)
}

/// What submitting the post form did.
#[derive(Debug, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum WritePostReply {
    /// The post is in the file.
    Written,
    /// The form was invalid; it comes back with its errors showing.
    Invalid { form: Shown },
}

/// Submits the post form: when it is valid, writes the post into the file,
/// in the communities ticked, and starts a sync in the background.
#[tauri::command]
pub async fn write_post(
    app: AppHandle,
    state: State<'_, AppState>,
    form: FormState,
    entries: Vec<(String, String)>,
) -> Result<WritePostReply, String> {
    let post = match forms::submit_post_form(&member_communities(&state)?, &form, &entries)? {
        Ok(post) => post,
        Err(form) => return Ok(WritePostReply::Invalid { form }),
    };
    write(app, &state, |connection, user_id, now| {
        store::write_post(connection, user_id, &post.body, &post.community_ids, now)
    })?;
    Ok(WritePostReply::Written)
}

#[tauri::command]
pub async fn edit_post(
    app: AppHandle,
    state: State<'_, AppState>,
    post_id: String,
    body: String,
) -> Result<(), String> {
    write(app, &state, |connection, user_id, now| {
        store::edit_post(connection, user_id, &post_id, &body, now)
    })
}

#[tauri::command]
pub async fn delete_post(
    app: AppHandle,
    state: State<'_, AppState>,
    post_id: String,
) -> Result<(), String> {
    write(app, &state, |connection, user_id, now| {
        store::delete_post(connection, user_id, &post_id, now)
    })
}

#[tauri::command]
pub async fn add_link(
    app: AppHandle,
    state: State<'_, AppState>,
    post_id: String,
    label: String,
    url: String,
) -> Result<(), String> {
    write(app, &state, |connection, user_id, now| {
        posting::add_link(connection, user_id, &post_id, &label, &url, now).map(drop)
    })
}

#[tauri::command]
pub async fn remove_link(
    app: AppHandle,
    state: State<'_, AppState>,
    link_id: String,
) -> Result<(), String> {
    write(app, &state, |connection, user_id, now| {
        posting::remove_link(connection, user_id, &link_id, now)
    })
}

#[tauri::command]
pub async fn tag_post(
    app: AppHandle,
    state: State<'_, AppState>,
    post_id: String,
    name: String,
) -> Result<(), String> {
    write(app, &state, |connection, user_id, now| {
        tagging::tag_post(connection, user_id, &post_id, &name, now)
    })
}

/// Takes a tag off a post, by its `app_posts_tags` row.
#[tauri::command]
pub async fn untag_post(
    app: AppHandle,
    state: State<'_, AppState>,
    posts_tags_id: String,
) -> Result<(), String> {
    write(app, &state, |connection, user_id, now| {
        tagging::untag(connection, user_id, &posts_tags_id, now)
    })
}

// Tags

#[tauri::command]
pub fn get_tags_page(state: State<'_, AppState>) -> Result<TagsPage, String> {
    let (connection, user_id) = open(&state.device)?;
    pages::tags_page(&connection, &user_id)
}

#[tauri::command]
pub async fn create_tag(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<(), String> {
    write(app, &state, |connection, user_id, now| {
        tagging::create_tag(connection, user_id, &name, now).map(drop)
    })
}

#[tauri::command]
pub async fn rename_tag(
    app: AppHandle,
    state: State<'_, AppState>,
    tag_id: String,
    name: String,
) -> Result<(), String> {
    write(app, &state, |connection, user_id, now| {
        tagging::rename_tag(connection, user_id, &tag_id, &name, now)
    })
}

#[tauri::command]
pub async fn delete_tag(
    app: AppHandle,
    state: State<'_, AppState>,
    tag_id: String,
) -> Result<(), String> {
    write(app, &state, |connection, user_id, now| {
        tagging::delete_tag(connection, user_id, &tag_id, now)
    })
}

// Profile

#[tauri::command]
pub fn get_profile_page(state: State<'_, AppState>) -> Result<ProfilePage, String> {
    let (connection, user_id) = open(&state.device)?;
    pages::profile_page(&connection, &user_id)
}

#[tauri::command]
pub async fn edit_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    display_name: String,
    bio: String,
) -> Result<(), String> {
    write(app, &state, |connection, user_id, now| {
        store::edit_profile(connection, user_id, &display_name, &bio, now)
    })
}
