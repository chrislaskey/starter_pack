//! Posts: the first resource.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::database::Database;
use crate::sqlite;

/// A post as the pages show it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Post {
    pub id: Uuid,
    pub body: String,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum PostError {
    #[error("post {id} not found")]
    NotFound { id: Uuid },
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Every post, newest first.
pub async fn list_posts(database: &Database) -> Result<Vec<Post>, PostError> {
    match database {
        Database::Sqlite(pool) => Ok(sqlite::posts::list_posts(pool).await?),
    }
}

/// One post by its id.
pub async fn get_post(database: &Database, id: Uuid) -> Result<Post, PostError> {
    let post = match database {
        Database::Sqlite(pool) => sqlite::posts::get_post(pool, id).await?,
    };
    post.ok_or(PostError::NotFound { id })
}

/// Writes a new post, with an id and the time from this device.
pub async fn create_post(database: &Database, body: String) -> Result<Post, PostError> {
    let now = Utc::now();
    let post = Post {
        id: Uuid::now_v7(),
        body,
        inserted_at: now,
        updated_at: now,
    };
    match database {
        Database::Sqlite(pool) => sqlite::posts::create_post(pool, &post).await?,
    }
    Ok(post)
}

/// Writes three sample posts when there are none, so a fresh database has
/// something to show. A stand-in until the pages can write posts.
pub async fn seed_sample_posts(database: &Database) -> Result<(), PostError> {
    let count = match database {
        Database::Sqlite(pool) => sqlite::posts::count_posts(pool).await?,
    };
    if count > 0 {
        return Ok(());
    }
    for body in [
        "Hello from app_core.",
        "The web server answers over HTTP, the apps over Tauri commands, and both call app_core.",
        "The same Svelte pages run in the browser, on the desktop and on a phone.",
    ] {
        create_post(database, body.to_string()).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn database() -> Database {
        Database::open_sqlite_in_memory()
            .await
            .expect("an in-memory database")
    }

    #[tokio::test]
    async fn lists_posts_newest_first() {
        let database = database().await;
        let first = create_post(&database, "first".to_string())
            .await
            .expect("a post");
        let second = create_post(&database, "second".to_string())
            .await
            .expect("a post");

        let posts = list_posts(&database).await.expect("the posts");
        assert_eq!(posts, vec![second, first]);
    }

    #[tokio::test]
    async fn gets_a_post_by_id() {
        let database = database().await;
        let written = create_post(&database, "a post".to_string())
            .await
            .expect("a post");

        let post = get_post(&database, written.id).await.expect("the post");
        assert_eq!(post, written);
    }

    #[tokio::test]
    async fn reports_a_missing_post() {
        let database = database().await;
        let id = Uuid::now_v7();

        let error = get_post(&database, id).await.expect_err("no such post");
        assert!(matches!(error, PostError::NotFound { id: missing } if missing == id));
    }

    #[tokio::test]
    async fn seeds_sample_posts_once() {
        let database = database().await;
        seed_sample_posts(&database).await.expect("seeded");
        seed_sample_posts(&database).await.expect("seeded again");

        assert_eq!(list_posts(&database).await.expect("the posts").len(), 3);
    }
}
