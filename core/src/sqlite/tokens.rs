//! The `users_tokens` table: a token per row, matched exactly.

use sqlx::SqliteConnection;

use crate::tokens::Token;

pub async fn insert(connection: &mut SqliteConnection, token: &Token) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO users_tokens (id, user_id, token, inserted_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(token.id.hyphenated())
    .bind(token.user_id.hyphenated())
    .bind(&token.token)
    .bind(token.inserted_at)
    .execute(connection)
    .await?;
    Ok(())
}
