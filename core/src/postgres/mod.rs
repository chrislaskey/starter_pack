//! The PostgreSQL database: its migrations, embedded from
//! `migrations/postgres`, and one module of plain SQL per table, each
//! the counterpart of the one under `sqlite`.

use sqlx::migrate::Migrator;

pub mod device_settings;
pub mod events;
pub mod posts;
pub mod sync_state;

pub static MIGRATOR: Migrator = sqlx::migrate!("migrations/postgres");
