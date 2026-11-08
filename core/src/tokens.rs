//! API tokens: what a device sends as a bearer token on every request to
//! the server, which resolves it to a user, `users::get_by_token`. Made
//! by the server at a login, later; until then by hand or in tests.

use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

use crate::database::sql;
use crate::deployment::{Deployment, Role, now};

/// A token as `users_tokens` keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token: String,
    pub inserted_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
pub enum TokenError {
    #[error("only the server makes tokens")]
    ServerOnly,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Makes a token for the user, on the server, and returns it: 32 random
/// bytes as hex, kept as is and matched exactly.
pub async fn create(deployment: &Deployment, user_id: Uuid) -> Result<String, TokenError> {
    if deployment.role != Role::Server {
        return Err(TokenError::ServerOnly);
    }
    let mut bytes = [0u8; 32];
    rand::fill(&mut bytes);
    let token = Token {
        id: Uuid::now_v7(),
        user_id,
        token: bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
        inserted_at: now(),
    };
    let mut tx = deployment.begin().await?;
    sql!(&mut tx, tokens::insert(&token))?;
    tx.commit().await?;
    Ok(token.token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::with_each_database;
    use crate::users::{self, UserError};

    #[tokio::test]
    async fn tokens_create_stands_for_its_user() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let alice = users::create(&deployment, "alice@example.com")
                .await
                .expect("a user");
            let token = create(&deployment, alice.id).await.expect("a token");
            assert_eq!(token.len(), 64);

            let user = users::get_by_token(&deployment, &token)
                .await
                .expect("the user");
            assert_eq!(user, alice);
            let error = users::get_by_token(&deployment, "not-a-token")
                .await
                .expect_err("unknown");
            assert!(matches!(error, UserError::UnknownToken));
        })
        .await;
    }

    #[tokio::test]
    async fn tokens_create_is_the_servers_alone() {
        let deployment = users::tests::device().await;
        let error = create(&deployment, Uuid::now_v7())
            .await
            .expect_err("a device");
        assert!(matches!(error, TokenError::ServerOnly));
    }
}
