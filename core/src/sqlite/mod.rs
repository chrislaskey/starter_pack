//! The SQLite database: its migrations, embedded from `migrations/sqlite`,
//! and one module of plain SQL per resource.

use sqlx::migrate::Migrator;

pub mod posts;

pub static MIGRATOR: Migrator = sqlx::migrate!("migrations/sqlite");
