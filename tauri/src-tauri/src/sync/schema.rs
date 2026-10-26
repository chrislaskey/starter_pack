//! The device's SQLite file: the server's client migrations,
//! embedded at build time from `phoenix/priv/client_migrations` by
//! `build.rs`, plus the two client-only tables its README describes.

use std::fs;
use std::path::Path;

use rusqlite::Connection;

include!(concat!(env!("OUT_DIR"), "/client_migrations.rs"));

const CLIENT_TABLES: &str = r#"
CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  inserted_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS sync_state (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  last_pulled_at TEXT,
  last_pulled_id TEXT,
  server_purge_horizon TEXT
);
INSERT OR IGNORE INTO sync_state (id) VALUES (1);
CREATE TABLE IF NOT EXISTS sync_outbox (
  table_name TEXT NOT NULL,
  row_id TEXT NOT NULL,
  queued_at TEXT NOT NULL,
  attempts INTEGER NOT NULL DEFAULT 0,
  last_error TEXT,
  PRIMARY KEY (table_name, row_id)
);
CREATE TABLE IF NOT EXISTS sync_backfills (
  community_id TEXT PRIMARY KEY
);
"#;

/// Opens the file, creating and migrating it when needed.
pub fn open(path: &Path) -> Result<Connection, String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    let connection = Connection::open(path).map_err(|error| error.to_string())?;
    prepare(connection)
}

/// An in-memory file, for tests.
#[cfg(test)]
pub fn open_in_memory() -> Result<Connection, String> {
    let connection = Connection::open_in_memory().map_err(|error| error.to_string())?;
    prepare(connection)
}

fn prepare(connection: Connection) -> Result<Connection, String> {
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|error| error.to_string())?;
    connection
        .execute_batch(CLIENT_TABLES)
        .map_err(|error| error.to_string())?;

    for (version, name, sql) in CLIENT_MIGRATIONS {
        let applied: bool = connection
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = ?1)",
                [version],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if applied {
            continue;
        }

        let batch = format!(
            "BEGIN;\n{sql}\nINSERT INTO schema_migrations (version, inserted_at) VALUES ({version}, '{}');\nCOMMIT;",
            super::now_iso8601()
        );
        connection
            .execute_batch(&batch)
            .map_err(|error| format!("client migration {name} failed: {error}"))?;
    }

    Ok(connection)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_every_embedded_migration_once() {
        let connection = open_in_memory().unwrap();

        let applied: i64 = connection
            .query_row("SELECT count(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(applied as usize, CLIENT_MIGRATIONS.len());
        assert!(!CLIENT_MIGRATIONS.is_empty());

        let tables: Vec<String> = connection
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for table in [
            "app_users",
            "app_communities",
            "app_users_communities",
            "app_posts",
            "app_posts_communities",
            "sync_state",
            "sync_outbox",
        ] {
            assert!(
                tables.contains(&table.to_string()),
                "{table} is missing from {tables:?}"
            );
        }

        // Opening again changes nothing
        prepare(connection).unwrap();
    }
}
