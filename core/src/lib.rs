//! The application: what the web server (`../axum`) and the Tauri desktop
//! and mobile app (`../tauri`) both run. One module per resource, with
//! `list`, `get`, `create`, `update` and `delete` functions called through
//! the module, `posts::list`, that return plain data the UI shows. They
//! read and write the `Deployment` that opened the database, see
//! `deployment`; the server and the app only translate between their
//! transport and these functions.
//!
//! Every write is an event, see `events`, which is how the apps' local
//! databases and the web server converge, see `sync` and
//! `docs/design/offline-sync.md`.

pub mod conflicts;
pub mod database;
pub mod deployment;
pub mod device_settings;
pub mod events;
#[cfg(feature = "postgres")]
mod postgres;
pub mod posts;
mod sqlite;
pub mod sync;
