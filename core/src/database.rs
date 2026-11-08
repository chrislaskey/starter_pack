//! The database a deployment opens: a SQLite file on the device for the
//! desktop and mobile apps, and for the web server too by default, or
//! PostgreSQL for the web server, behind the `postgres` feature. Opening
//! runs the migrations under `migrations/`, so every deployment has the
//! same tables. A core function runs in one `Transaction` on it, and
//! through the `sql!` macro each statement runs in the database's own SQL
//! module, see `sqlite` and `postgres`.

use std::path::Path;
use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use thiserror::Error;

use crate::sqlite;

#[derive(Clone, Debug)]
pub enum Database {
    Sqlite(SqlitePool),
    #[cfg(feature = "postgres")]
    Postgres(sqlx::PgPool),
}

/// One transaction: what every core function runs in, so a row and the
/// event that changed it are written together or not at all. The SQL
/// modules take the connection inside it, never a pool.
pub enum Transaction {
    Sqlite(sqlx::Transaction<'static, sqlx::Sqlite>),
    #[cfg(feature = "postgres")]
    Postgres(sqlx::Transaction<'static, sqlx::Postgres>),
}

/// Runs one statement of a SQL module on the transaction's connection:
/// `sql!(tx, posts::list())` is `sqlite::posts::list(connection)` on
/// SQLite and `postgres::posts::list(connection)` on PostgreSQL, so a
/// resource's rules are written once and the SQL once per database.
macro_rules! sql {
    ($tx:expr, $module:ident :: $function:ident ( $($argument:expr),* $(,)? )) => {
        match $tx {
            $crate::database::Transaction::Sqlite(tx) => {
                $crate::sqlite::$module::$function(&mut **tx, $($argument),*).await
            }
            #[cfg(feature = "postgres")]
            $crate::database::Transaction::Postgres(tx) => {
                $crate::postgres::$module::$function(&mut **tx, $($argument),*).await
            }
        }
    };
}
pub(crate) use sql;

impl Transaction {
    pub async fn commit(self) -> Result<(), sqlx::Error> {
        match self {
            Transaction::Sqlite(tx) => tx.commit().await,
            #[cfg(feature = "postgres")]
            Transaction::Postgres(tx) => tx.commit().await,
        }
    }
}

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("could not open the database: {0}")]
    Open(#[source] sqlx::Error),
    #[error("could not migrate the database: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

impl Database {
    pub async fn begin(&self) -> Result<Transaction, sqlx::Error> {
        Ok(match self {
            Database::Sqlite(pool) => Transaction::Sqlite(pool.begin().await?),
            #[cfg(feature = "postgres")]
            Database::Postgres(pool) => Transaction::Postgres(pool.begin().await?),
        })
    }

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

    /// Connects to the PostgreSQL database at the URL, which must exist,
    /// and brings it up to date.
    #[cfg(feature = "postgres")]
    pub async fn open_postgres(url: &str) -> Result<Database, DatabaseError> {
        let options =
            sqlx::postgres::PgConnectOptions::from_str(url).map_err(DatabaseError::Open)?;
        Self::open_postgres_with(options).await
    }

    #[cfg(feature = "postgres")]
    async fn open_postgres_with(
        options: sqlx::postgres::PgConnectOptions,
    ) -> Result<Database, DatabaseError> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_with(options)
            .await
            .map_err(DatabaseError::Open)?;
        crate::postgres::MIGRATOR.run(&pool).await?;
        Ok(Database::Postgres(pool))
    }
}

/// Runs a test against every database the tests can reach: SQLite in
/// memory always, and PostgreSQL when `TEST_POSTGRES_URL` names a database
/// to use, each test in a schema of its own that is dropped afterwards.
/// Every resource test runs through here, so the two databases' modules
/// never drift apart.
#[cfg(test)]
pub(crate) async fn with_each_database<Test, Run>(test: Test)
where
    Test: Fn(Database) -> Run,
    Run: Future<Output = ()>,
{
    let sqlite = Database::open_sqlite_in_memory()
        .await
        .expect("an in-memory SQLite database");
    test(sqlite).await;

    #[cfg(feature = "postgres")]
    if let Ok(url) = std::env::var("TEST_POSTGRES_URL") {
        let schema = format!("test_{}", uuid::Uuid::now_v7().simple());
        let options = sqlx::postgres::PgConnectOptions::from_str(&url)
            .expect("TEST_POSTGRES_URL is a PostgreSQL URL")
            .options([("search_path", schema.as_str())]);
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_with(options.clone())
            .await
            .expect("a connection to TEST_POSTGRES_URL");
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
            .execute(&pool)
            .await
            .expect("a schema for the test");
        let postgres = Database::open_postgres_with(options)
            .await
            .expect("a migrated PostgreSQL schema");
        test(postgres).await;
        sqlx::query(sqlx::AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
            .execute(&pool)
            .await
            .expect("the schema dropped");
    }
}
