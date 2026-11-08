//! The `users_tokens` table, the counterpart of `sqlite::tokens`.

use sqlx::PgConnection;

use crate::tokens::Token;

pub async fn insert(connection: &mut PgConnection, token: &Token) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO users_tokens (id, user_id, token, inserted_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(token.id)
    .bind(token.user_id)
    .bind(&token.token)
    .bind(token.inserted_at)
    .execute(connection)
    .await?;
    Ok(())
}
