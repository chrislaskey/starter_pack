//! The SQLite database: its migrations, embedded from `migrations/sqlite`,
//! and one module of plain SQL per table, each function on the
//! connection of the transaction a core function runs in.

use sqlx::migrate::Migrator;

pub mod device_settings;
pub mod events;
pub mod posts;
pub mod sync_state;

pub static MIGRATOR: Migrator = sqlx::migrate!("migrations/sqlite");
