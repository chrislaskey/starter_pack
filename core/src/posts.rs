//! Posts: the first resource. A post is some text, written, edited and
//! deleted from any deployment. Called through the module, `posts::list`,
//! as the standard library's `fs::read` is.

use chrono::{DateTime, SubsecRound, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::database::Database;
#[cfg(feature = "postgres")]
use crate::postgres;
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
    #[error("a post needs some text")]
    BlankBody,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Every post, newest first.
pub async fn list(database: &Database) -> Result<Vec<Post>, PostError> {
    match database {
        Database::Sqlite(pool) => Ok(sqlite::posts::list(pool).await?),
        #[cfg(feature = "postgres")]
        Database::Postgres(pool) => Ok(postgres::posts::list(pool).await?),
    }
}

/// One post by its id.
pub async fn get(database: &Database, id: Uuid) -> Result<Post, PostError> {
    let post = match database {
        Database::Sqlite(pool) => sqlite::posts::get(pool, id).await?,
        #[cfg(feature = "postgres")]
        Database::Postgres(pool) => postgres::posts::get(pool, id).await?,
    };
    post.ok_or(PostError::NotFound { id })
}

/// Writes a new post, with an id and the time from this device.
pub async fn create(database: &Database, body: &str) -> Result<Post, PostError> {
    let now = now();
    let post = Post {
        id: Uuid::now_v7(),
        body: checked_body(body)?,
        inserted_at: now,
        updated_at: now,
    };
    match database {
        Database::Sqlite(pool) => sqlite::posts::create(pool, &post).await?,
        #[cfg(feature = "postgres")]
        Database::Postgres(pool) => postgres::posts::create(pool, &post).await?,
    }
    Ok(post)
}

/// Writes a new body over the post, with the time from this device.
pub async fn update(database: &Database, id: Uuid, body: &str) -> Result<Post, PostError> {
    let mut post = get(database, id).await?;
    post.body = checked_body(body)?;
    post.updated_at = now();
    let updated = match database {
        Database::Sqlite(pool) => sqlite::posts::update(pool, &post).await?,
        #[cfg(feature = "postgres")]
        Database::Postgres(pool) => postgres::posts::update(pool, &post).await?,
    };
    if !updated {
        return Err(PostError::NotFound { id });
    }
    Ok(post)
}

/// Deletes the post.
pub async fn delete(database: &Database, id: Uuid) -> Result<(), PostError> {
    let deleted = match database {
        Database::Sqlite(pool) => sqlite::posts::delete(pool, id).await?,
        #[cfg(feature = "postgres")]
        Database::Postgres(pool) => postgres::posts::delete(pool, id).await?,
    };
    if !deleted {
        return Err(PostError::NotFound { id });
    }
    Ok(())
}

/// The body as it is kept: trimmed, and not blank.
fn checked_body(body: &str) -> Result<String, PostError> {
    let body = body.trim();
    if body.is_empty() {
        return Err(PostError::BlankBody);
    }
    Ok(body.to_string())
}

/// The time now, to the microsecond, which is what the databases keep.
fn now() -> DateTime<Utc> {
    Utc::now().trunc_subsecs(6)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::with_each_database;

    #[tokio::test]
    async fn posts_list_newest_first() {
        with_each_database(|database| async move {
            let first = create(&database, "first").await.expect("a post");
            let second = create(&database, "second").await.expect("a post");

            let posts = list(&database).await.expect("the posts");
            assert_eq!(posts, vec![second, first]);
        })
        .await;
    }

    #[tokio::test]
    async fn posts_get_by_id() {
        with_each_database(|database| async move {
            let written = create(&database, "a post").await.expect("a post");

            let post = get(&database, written.id).await.expect("the post");
            assert_eq!(post, written);
        })
        .await;
    }

    #[tokio::test]
    async fn posts_get_missing() {
        with_each_database(|database| async move {
            let id = Uuid::now_v7();

            let error = get(&database, id).await.expect_err("no such post");
            assert!(matches!(error, PostError::NotFound { id: missing } if missing == id));
        })
        .await;
    }

    #[tokio::test]
    async fn posts_create_trims_and_refuses_blank() {
        with_each_database(|database| async move {
            let post = create(&database, "  spaced  ").await.expect("a post");
            assert_eq!(post.body, "spaced");

            let error = create(&database, " \n ").await.expect_err("blank");
            assert!(matches!(error, PostError::BlankBody));
            assert_eq!(list(&database).await.expect("the posts").len(), 1);
        })
        .await;
    }

    #[tokio::test]
    async fn posts_update() {
        with_each_database(|database| async move {
            let written = create(&database, "before").await.expect("a post");

            let updated = update(&database, written.id, "after")
                .await
                .expect("updated");
            assert_eq!(updated.body, "after");
            assert_eq!(updated.inserted_at, written.inserted_at);
            assert!(updated.updated_at >= written.updated_at);
            assert_eq!(get(&database, written.id).await.expect("the post"), updated);

            let error = update(&database, written.id, "").await.expect_err("blank");
            assert!(matches!(error, PostError::BlankBody));
            let error = update(&database, Uuid::now_v7(), "x")
                .await
                .expect_err("missing");
            assert!(matches!(error, PostError::NotFound { .. }));
        })
        .await;
    }

    #[tokio::test]
    async fn posts_delete() {
        with_each_database(|database| async move {
            let written = create(&database, "going").await.expect("a post");

            delete(&database, written.id).await.expect("deleted");
            let error = get(&database, written.id).await.expect_err("gone");
            assert!(matches!(error, PostError::NotFound { .. }));
            let error = delete(&database, written.id)
                .await
                .expect_err("already gone");
            assert!(matches!(error, PostError::NotFound { .. }));
        })
        .await;
    }
}
