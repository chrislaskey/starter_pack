//! The `sync_state` table: a device's pull cursor, one row, written by
//! the first pull. The server never writes it; this is the counterpart
//! of `sqlite::sync_state`.

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::sync::Cursor;

pub async fn cursor(connection: &mut PgConnection) -> Result<Option<Cursor>, sqlx::Error> {
    let row: Option<(DateTime<Utc>, Uuid)> =
        sqlx::query_as("SELECT cursor_last_synced_at, cursor_id FROM sync_state WHERE id = 1")
            .fetch_optional(connection)
            .await?;
    Ok(row.map(|(last_synced_at, id)| Cursor { last_synced_at, id }))
}

pub async fn save_cursor(
    connection: &mut PgConnection,
    cursor: &Cursor,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO sync_state (id, cursor_last_synced_at, cursor_id) VALUES (1, $1, $2)
         ON CONFLICT (id) DO UPDATE SET
           cursor_last_synced_at = excluded.cursor_last_synced_at, cursor_id = excluded.cursor_id",
    )
    .bind(cursor.last_synced_at)
    .bind(cursor.id)
    .execute(connection)
    .await?;
    Ok(())
}
