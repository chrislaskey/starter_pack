//! The `users_tokens` table: a token's hash per row, matched exactly.

use chrono::{DateTime, Utc};
use sqlx::SqliteConnection;

use crate::tokens::Token;

pub async fn insert(connection: &mut SqliteConnection, token: &Token) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO users_tokens (id, user_id, token_hash, inserted_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(token.id.hyphenated())
    .bind(token.user_id.hyphenated())
    .bind(&token.token_hash)
    .bind(token.inserted_at)
    .execute(connection)
    .await?;
    Ok(())
}

/// Deletes the token with the hash; whether there was one.
pub async fn delete_by_hash(
    connection: &mut SqliteConnection,
    token_hash: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM users_tokens WHERE token_hash = $1")
        .bind(token_hash)
        .execute(connection)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Deletes the tokens made before the time, which have expired.
pub async fn delete_made_before(
    connection: &mut SqliteConnection,
    time: DateTime<Utc>,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM users_tokens WHERE inserted_at < $1")
        .bind(time)
        .execute(connection)
        .await?;
    Ok(result.rows_affected())
}
