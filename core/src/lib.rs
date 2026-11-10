//! The application: what the web server (`../axum`) and the Tauri desktop
//! and mobile app (`../tauri`) both run. One module per resource, with
//! `list`, `get`, `create`, `update` and `delete` functions called through
//! the module, `posts::list`, that return plain data the UI shows. They
//! read and write the `Deployment` that opened the database, see
//! `deployment`; the server and the app only translate between their
//! transport and these functions.
//!
//! Every function that reads or writes a resource takes the `users::User`
//! acting, who owns what they write and sees what is theirs or in a
//! community of theirs. Every write is an event, see `events`, which is
//! how the apps' local databases and the web server converge, see `sync`
//! and `docs/design/offline-sync.md`.

pub mod accounts;
pub mod avatars;
pub mod communities;
pub mod conflicts;
pub mod credentials;
pub mod current_community;
pub mod database;
pub mod deployment;
pub mod device_settings;
pub mod events;
pub mod field_errors;
pub mod files;
pub mod jobs;
pub mod mailer;
pub mod mails;
pub mod memberships;
pub mod pending_changes;
#[cfg(feature = "postgres")]
mod postgres;
pub mod posts;
pub mod posts_communities;
pub mod purge;
mod sqlite;
#[cfg(feature = "object-storage")]
pub mod storage;
pub mod sync;
pub mod tokens;
pub mod users;
