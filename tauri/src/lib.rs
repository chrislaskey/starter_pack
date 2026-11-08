//! The desktop and mobile app: one window showing the Svelte pages
//! (`../svelte`), and a Tauri command per `app_core` function the pages
//! call, one module per resource, each command named resource first,
//! `posts_list`, since the commands are one namespace. The app opens its own SQLite file, see
//! `database`, the commands read and write it, and it syncs with the web
//! server, see `sync`.

mod conflicts;
mod database;
mod posts;
mod sync;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            database::open_and_manage(app)?;
            sync::setup(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
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
            conflicts::conflicts_let_go
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
