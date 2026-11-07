//! The `app_posts` table. Ids are UUIDv7 text, so the file reads well in
//! the sqlite3 shell; the row type here turns them into `Post`'s `Uuid`.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;
use uuid::fmt::Hyphenated;

use crate::posts::Post;

#[derive(FromRow)]
struct PostRow {
    id: Hyphenated,
    body: String,
    inserted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<PostRow> for Post {
    fn from(row: PostRow) -> Post {
        Post {
            id: row.id.into_uuid(),
            body: row.body,
            inserted_at: row.inserted_at,
            updated_at: row.updated_at,
        }
    }
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<Post>, sqlx::Error> {
    let rows: Vec<PostRow> =
        sqlx::query_as("SELECT id, body, inserted_at, updated_at FROM app_posts ORDER BY inserted_at DESC, id DESC")
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(Post::from).collect())
}

pub async fn get(pool: &SqlitePool, id: Uuid) -> Result<Option<Post>, sqlx::Error> {
    let row: Option<PostRow> =
        sqlx::query_as("SELECT id, body, inserted_at, updated_at FROM app_posts WHERE id = $1")
            .bind(id.hyphenated())
            .fetch_optional(pool)
            .await?;
    Ok(row.map(Post::from))
}

pub async fn create(pool: &SqlitePool, post: &Post) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_posts (id, body, inserted_at, updated_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(post.id.hyphenated())
    .bind(&post.body)
    .bind(post.inserted_at)
    .bind(post.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Writes the post's body and `updated_at` over the row; false when there is no such row.
pub async fn update(pool: &SqlitePool, post: &Post) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("UPDATE app_posts SET body = $2, updated_at = $3 WHERE id = $1")
        .bind(post.id.hyphenated())
        .bind(&post.body)
        .bind(post.updated_at)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() == 1)
}

/// Deletes the row; false when there is no such row.
pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM app_posts WHERE id = $1")
        .bind(id.hyphenated())
        .execute(pool)
        .await?;
    Ok(result.rows_affected() == 1)
}
