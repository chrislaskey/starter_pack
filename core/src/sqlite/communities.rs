//! The `app_communities` table.

use chrono::{DateTime, Utc};
use sqlx::types::Json;
use sqlx::{FromRow, SqliteConnection};
use uuid::Uuid;
use uuid::fmt::Hyphenated;

use crate::communities::Community;
use crate::sync::Cursor;

#[derive(FromRow)]
struct CommunityRow {
    id: Hyphenated,
    name: String,
    inserted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    version: i64,
    last_synced_at: Option<DateTime<Utc>>,
    deleted_at: Option<DateTime<Utc>>,
    deleted_by_id: Option<Hyphenated>,
}

impl From<CommunityRow> for Community {
    fn from(row: CommunityRow) -> Community {
        Community {
            id: row.id.into_uuid(),
            name: row.name,
            inserted_at: row.inserted_at,
            updated_at: row.updated_at,
            version: row.version,
            last_synced_at: row.last_synced_at,
            deleted_at: row.deleted_at,
            deleted_by_id: row.deleted_by_id.map(Hyphenated::into_uuid),
        }
    }
}

macro_rules! select {
    ($($rest:expr),+ $(,)?) => {
        concat!(
            "SELECT id, name, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id FROM app_communities ",
            $($rest),+
        )
    };
}

/// Every community not deleted, by name.
pub async fn list_live(connection: &mut SqliteConnection) -> Result<Vec<Community>, sqlx::Error> {
    let rows: Vec<CommunityRow> =
        sqlx::query_as(select!("WHERE deleted_at IS NULL ORDER BY name, id"))
            .fetch_all(connection)
            .await?;
    Ok(rows.into_iter().map(Community::from).collect())
}

/// The rows with the ids, deleted or not.
pub async fn list_by_ids(
    connection: &mut SqliteConnection,
    ids: &[Uuid],
) -> Result<Vec<Community>, sqlx::Error> {
    let rows: Vec<CommunityRow> =
        sqlx::query_as(select!("WHERE id IN (SELECT value FROM json_each($1))"))
            .bind(Json(
                ids.iter()
                    .map(|id| id.hyphenated().to_string())
                    .collect::<Vec<String>>(),
            ))
            .fetch_all(connection)
            .await?;
    Ok(rows.into_iter().map(Community::from).collect())
}

/// The row by its id, deleted or not.
pub async fn get(
    connection: &mut SqliteConnection,
    id: Uuid,
) -> Result<Option<Community>, sqlx::Error> {
    let row: Option<CommunityRow> = sqlx::query_as(select!("WHERE id = $1"))
        .bind(id.hyphenated())
        .fetch_optional(connection)
        .await?;
    Ok(row.map(Community::from))
}

/// The rows the server wrote after the cursor, in cursor order, deleted
/// or not: a page of a pull. Every user sees every community.
pub async fn list_synced_after(
    connection: &mut SqliteConnection,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<Community>, sqlx::Error> {
    let rows: Vec<CommunityRow> = sqlx::query_as(select!(
        "WHERE last_synced_at IS NOT NULL AND (last_synced_at, id) > ($1, $2)
         ORDER BY last_synced_at, id LIMIT $3"
    ))
    .bind(after.last_synced_at)
    .bind(after.id.hyphenated())
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(Community::from).collect())
}

pub async fn insert(
    connection: &mut SqliteConnection,
    community: &Community,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_communities (id, name, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(community.id.hyphenated())
    .bind(&community.name)
    .bind(community.inserted_at)
    .bind(community.updated_at)
    .bind(community.version)
    .bind(community.last_synced_at)
    .bind(community.deleted_at)
    .bind(community.deleted_by_id.map(Uuid::hyphenated))
    .execute(connection)
    .await?;
    Ok(())
}

pub async fn update(
    connection: &mut SqliteConnection,
    community: &Community,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE app_communities SET name = $2, updated_at = $3, version = $4, last_synced_at = $5, deleted_at = $6, deleted_by_id = $7
         WHERE id = $1",
    )
    .bind(community.id.hyphenated())
    .bind(&community.name)
    .bind(community.updated_at)
    .bind(community.version)
    .bind(community.last_synced_at)
    .bind(community.deleted_at)
    .bind(community.deleted_by_id.map(Uuid::hyphenated))
    .execute(connection)
    .await?;
    Ok(())
}

/// Writes the server's row over the device's, or inserts it.
pub async fn upsert(
    connection: &mut SqliteConnection,
    community: &Community,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_communities (id, name, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (id) DO UPDATE SET
           name = excluded.name, inserted_at = excluded.inserted_at, updated_at = excluded.updated_at,
           version = excluded.version, last_synced_at = excluded.last_synced_at,
           deleted_at = excluded.deleted_at, deleted_by_id = excluded.deleted_by_id",
    )
    .bind(community.id.hyphenated())
    .bind(&community.name)
    .bind(community.inserted_at)
    .bind(community.updated_at)
    .bind(community.version)
    .bind(community.last_synced_at)
    .bind(community.deleted_at)
    .bind(community.deleted_by_id.map(Uuid::hyphenated))
    .execute(connection)
    .await?;
    Ok(())
}

/// Removes the row itself, when a device's create was refused by the
/// server.
pub async fn delete_row(connection: &mut SqliteConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM app_communities WHERE id = $1")
        .bind(id.hyphenated())
        .execute(connection)
        .await?;
    Ok(())
}
