//! The `app_posts` table. Ids are UUIDv7 text, so the file reads well in
//! the sqlite3 shell; the row type here turns them into `Post`'s `Uuid`.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqliteConnection};
use uuid::Uuid;
use uuid::fmt::Hyphenated;

use crate::posts::Post;
use crate::sync::Cursor;

#[derive(FromRow)]
struct PostRow {
    id: Hyphenated,
    body: String,
    inserted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    version: i64,
    last_synced_at: Option<DateTime<Utc>>,
    deleted_at: Option<DateTime<Utc>>,
}

impl From<PostRow> for Post {
    fn from(row: PostRow) -> Post {
        Post {
            id: row.id.into_uuid(),
            body: row.body,
            inserted_at: row.inserted_at,
            updated_at: row.updated_at,
            version: row.version,
            last_synced_at: row.last_synced_at,
            deleted_at: row.deleted_at,
        }
    }
}

macro_rules! select {
    ($rest:literal) => {
        concat!(
            "SELECT id, body, inserted_at, updated_at, version, last_synced_at, deleted_at FROM app_posts ",
            $rest
        )
    };
}

/// The posts not deleted, newest first.
pub async fn list(connection: &mut SqliteConnection) -> Result<Vec<Post>, sqlx::Error> {
    let rows: Vec<PostRow> = sqlx::query_as(select!(
        "WHERE deleted_at IS NULL ORDER BY inserted_at DESC, id DESC"
    ))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(Post::from).collect())
}

/// The row by its id, deleted or not.
pub async fn get(connection: &mut SqliteConnection, id: Uuid) -> Result<Option<Post>, sqlx::Error> {
    let row: Option<PostRow> = sqlx::query_as(select!("WHERE id = $1"))
        .bind(id.hyphenated())
        .fetch_optional(connection)
        .await?;
    Ok(row.map(Post::from))
}

/// The rows the server wrote after the cursor, in cursor order, deleted
/// or not: a page of a pull.
pub async fn list_synced_after(
    connection: &mut SqliteConnection,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<Post>, sqlx::Error> {
    let rows: Vec<PostRow> = sqlx::query_as(select!(
        "WHERE last_synced_at IS NOT NULL AND (last_synced_at, id) > ($1, $2)
         ORDER BY last_synced_at, id LIMIT $3"
    ))
    .bind(after.last_synced_at)
    .bind(after.id.hyphenated())
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(Post::from).collect())
}

pub async fn insert(connection: &mut SqliteConnection, post: &Post) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_posts (id, body, inserted_at, updated_at, version, last_synced_at, deleted_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(post.id.hyphenated())
    .bind(&post.body)
    .bind(post.inserted_at)
    .bind(post.updated_at)
    .bind(post.version)
    .bind(post.last_synced_at)
    .bind(post.deleted_at)
    .execute(connection)
    .await?;
    Ok(())
}

/// Writes the post over its row; false when there is no such row.
pub async fn update(connection: &mut SqliteConnection, post: &Post) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE app_posts SET body = $2, updated_at = $3, version = $4, last_synced_at = $5, deleted_at = $6
         WHERE id = $1",
    )
    .bind(post.id.hyphenated())
    .bind(&post.body)
    .bind(post.updated_at)
    .bind(post.version)
    .bind(post.last_synced_at)
    .bind(post.deleted_at)
    .execute(connection)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Writes the server's row over the device's, or inserts it.
pub async fn upsert(connection: &mut SqliteConnection, post: &Post) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_posts (id, body, inserted_at, updated_at, version, last_synced_at, deleted_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (id) DO UPDATE SET
           body = excluded.body, inserted_at = excluded.inserted_at, updated_at = excluded.updated_at,
           version = excluded.version, last_synced_at = excluded.last_synced_at, deleted_at = excluded.deleted_at",
    )
    .bind(post.id.hyphenated())
    .bind(&post.body)
    .bind(post.inserted_at)
    .bind(post.updated_at)
    .bind(post.version)
    .bind(post.last_synced_at)
    .bind(post.deleted_at)
    .execute(connection)
    .await?;
    Ok(())
}

/// Removes the row itself, when a device's create was refused by the
/// server; a delete the person makes is a soft delete, see `posts::delete`.
pub async fn delete_row(connection: &mut SqliteConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM app_posts WHERE id = $1")
        .bind(id.hyphenated())
        .execute(connection)
        .await?;
    Ok(())
}
