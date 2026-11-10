//! The PostgreSQL database: its migrations, embedded from
//! `migrations/postgres`, and one module of plain SQL per table, each
//! the counterpart of the one under `sqlite`.

use sqlx::migrate::Migrator;

pub mod communities;
pub mod credentials;
pub mod device_settings;
pub mod events;
pub mod files;
pub mod jobs;
pub mod memberships;
pub mod posts;
pub mod posts_communities;
pub mod purges;
pub mod sync_backfills;
pub mod sync_state;
pub mod tokens;
pub mod users;

pub static MIGRATOR: Migrator = sqlx::migrate!("migrations/postgres");
