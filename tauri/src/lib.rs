//! The desktop and mobile app: one window showing the Svelte pages
//! (`../svelte`), and a Tauri command per `app_core` function the pages
//! call, one module per resource, each command named resource first,
//! `posts_list`, since the commands are one namespace. The app opens its
//! own SQLite file, see `database`, the commands read and write it as the
//! user signed in, see `users` and `accounts`, and it syncs with the web
//! server, see `sync`.

mod accounts;
mod communities;
mod conflicts;
mod database;
mod memberships;
mod pending_changes;
mod posts;
mod sync;
mod users;

use tauri::{Manager, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            database::open_and_manage(app)?;
            sync::setup(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if came_to_foreground(event) {
                sync::spawn_on_foreground(window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            users::users_me,
            accounts::accounts_sign_up,
            accounts::accounts_sign_in,
            accounts::accounts_sign_out,
            communities::communities_list,
            communities::communities_create,
            memberships::memberships_join,
            memberships::memberships_leave,
            posts::posts_list,
            posts::posts_get,
            posts::posts_create,
            posts::posts_update,
            posts::posts_delete,
            sync::sync_run,
            sync::sync_status,
            sync::sync_set_offline,
            conflicts::conflicts_list,
            conflicts::conflicts_reapply,
            conflicts::conflicts_let_go,
            pending_changes::pending_changes_list
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// The window gained focus, or the app resumed on a phone, which Tauri
/// only reports on mobile builds.
fn came_to_foreground(event: &WindowEvent) -> bool {
    #[cfg(mobile)]
    if matches!(event, WindowEvent::Resumed) {
        return true;
    }
    matches!(event, WindowEvent::Focused(true))
}
