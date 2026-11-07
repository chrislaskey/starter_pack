//! The `app_posts` table.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::posts::Post;

#[derive(FromRow)]
struct PostRow {
    id: Uuid,
    body: String,
    inserted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<PostRow> for Post {
    fn from(row: PostRow) -> Post {
        Post {
            id: row.id,
            body: row.body,
            inserted_at: row.inserted_at,
            updated_at: row.updated_at,
        }
    }
}

pub async fn list_posts(pool: &PgPool) -> Result<Vec<Post>, sqlx::Error> {
    let rows: Vec<PostRow> =
        sqlx::query_as("SELECT id, body, inserted_at, updated_at FROM app_posts ORDER BY inserted_at DESC, id DESC")
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(Post::from).collect())
}

pub async fn get_post(pool: &PgPool, id: Uuid) -> Result<Option<Post>, sqlx::Error> {
    let row: Option<PostRow> =
        sqlx::query_as("SELECT id, body, inserted_at, updated_at FROM app_posts WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(Post::from))
}

pub async fn create_post(pool: &PgPool, post: &Post) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_posts (id, body, inserted_at, updated_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(post.id)
    .bind(&post.body)
    .bind(post.inserted_at)
    .bind(post.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn count_posts(pool: &PgPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM app_posts")
        .fetch_one(pool)
        .await
}
