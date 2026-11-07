//! The database a deployment opens: a SQLite file on the device for the
//! desktop and mobile apps, and for the web server too by default. Opening
//! runs the migrations under `migrations/`, so every deployment has the
//! same tables. The PostgreSQL variant for the web server joins the enum
//! when it is built; each resource module matches on the variant and
//! calls the database's own functions, see `sqlite`.

use std::path::Path;
use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use thiserror::Error;

use crate::sqlite;

#[derive(Clone, Debug)]
pub enum Database {
    Sqlite(SqlitePool),
}

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("could not open the database: {0}")]
    Open(#[source] sqlx::Error),
    #[error("could not migrate the database: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

impl Database {
    /// Opens the SQLite file, creating it and its directory when missing,
    /// and brings it up to date.
    pub async fn open_sqlite_file(path: &Path) -> Result<Database, DatabaseError> {
        if let Some(directory) = path.parent() {
            std::fs::create_dir_all(directory)
                .map_err(|error| DatabaseError::Open(error.into()))?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .connect_with(options)
            .await
            .map_err(DatabaseError::Open)?;
        Self::migrate_sqlite(pool).await
    }

    /// A SQLite database in memory, for tests. One connection, kept open:
    /// each SQLite connection to memory is a database of its own, so the
    /// pool holds a single one and never lets it go.
    pub async fn open_sqlite_in_memory() -> Result<Database, DatabaseError> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .map_err(DatabaseError::Open)?
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(options)
            .await
            .map_err(DatabaseError::Open)?;
        Self::migrate_sqlite(pool).await
    }

    async fn migrate_sqlite(pool: SqlitePool) -> Result<Database, DatabaseError> {
        sqlite::MIGRATOR.run(&pool).await?;
        Ok(Database::Sqlite(pool))
    }
}
