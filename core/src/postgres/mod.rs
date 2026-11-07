//! The PostgreSQL database: its migrations, embedded from
//! `migrations/postgres`, and one module of plain SQL per resource, each
//! the counterpart of the one under `sqlite`.

use sqlx::migrate::Migrator;

pub mod posts;

pub static MIGRATOR: Migrator = sqlx::migrate!("migrations/postgres");
