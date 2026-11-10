//! The SQLite database: its migrations, embedded from `migrations/sqlite`,
//! and one module of plain SQL per table, each function on the
//! connection of the transaction a core function runs in.

use sqlx::migrate::Migrator;

pub mod communities;
pub mod credentials;
pub mod device_settings;
pub mod events;
pub mod files;
pub mod memberships;
pub mod posts;
pub mod posts_communities;
pub mod purges;
pub mod sync_backfills;
pub mod sync_state;
pub mod tokens;
pub mod users;

pub static MIGRATOR: Migrator = sqlx::migrate!("migrations/sqlite");
