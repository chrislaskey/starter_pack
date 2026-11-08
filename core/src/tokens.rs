//! API tokens: what a device sends as a bearer token on every request to
//! the server, and what the browser's cookie holds, which the server
//! resolves to a user, `users::get_by_token`. Made by a sign-up or a
//! sign-in, deleted by a sign-out, see `accounts`. Only the token's
//! hash is kept, so reading the table gives no token that works; a token
//! expires `VALIDITY_DAYS` after it was made.

use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::database::{Transaction, sql};
use crate::deployment::now;

/// How long a token works, as Phoenix's API tokens do.
pub const VALIDITY_DAYS: i64 = 365;

/// A token as `users_tokens` keeps it: by its hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub inserted_at: DateTime<Utc>,
}

/// Makes a token for the user and returns it: 32 random bytes as hex,
/// kept as its hash and matched exactly.
pub(crate) async fn create(tx: &mut Transaction, user_id: Uuid) -> Result<String, sqlx::Error> {
    let mut bytes = [0u8; 32];
    rand::fill(&mut bytes);
    let token: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    let row = Token {
        id: Uuid::now_v7(),
        user_id,
        token_hash: hash(&token),
        inserted_at: now(),
    };
    sql!(&mut *tx, tokens::insert(&row))?;
    Ok(token)
}

/// Deletes the token, so it no longer stands for anyone; whether it did.
pub(crate) async fn delete(tx: &mut Transaction, token: &str) -> Result<bool, sqlx::Error> {
    sql!(&mut *tx, tokens::delete_by_hash(&hash(token)))
}

/// Deletes the tokens that have expired as of `now`, for the purge.
pub(crate) async fn delete_expired(
    tx: &mut Transaction,
    now: DateTime<Utc>,
) -> Result<u64, sqlx::Error> {
    sql!(
        &mut *tx,
        tokens::delete_made_before(made_after_to_be_valid(now))
    )
}

/// The time a token must have been made after to be valid at `now`.
pub(crate) fn made_after_to_be_valid(now: DateTime<Utc>) -> DateTime<Utc> {
    now - Duration::days(VALIDITY_DAYS)
}

/// The hash kept for a token: SHA-256 as hex.
pub(crate) fn hash(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::with_each_database;
    use crate::deployment::{Deployment, Role};
    use crate::users::{self, UserError};

    #[tokio::test]
    async fn tokens_create_stands_for_its_user_until_deleted() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let alice = users::create(&deployment, "alice@example.com")
                .await
                .expect("a user");
            let mut tx = deployment.begin().await.expect("a transaction");
            let token = create(&mut tx, alice.id).await.expect("a token");
            tx.commit().await.expect("committed");
            assert_eq!(token.len(), 64);

            let user = users::get_by_token(&deployment, &token)
                .await
                .expect("the user");
            assert_eq!(user, alice);
            let error = users::get_by_token(&deployment, "not-a-token")
                .await
                .expect_err("unknown");
            assert!(matches!(error, UserError::UnknownToken));

            let mut tx = deployment.begin().await.expect("a transaction");
            assert!(delete(&mut tx, &token).await.expect("deleted"));
            assert!(!delete(&mut tx, &token).await.expect("already gone"));
            tx.commit().await.expect("committed");
            let error = users::get_by_token(&deployment, &token)
                .await
                .expect_err("deleted");
            assert!(matches!(error, UserError::UnknownToken));
        })
        .await;
    }

    #[tokio::test]
    async fn tokens_are_kept_hashed_and_expire() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let alice = users::create(&deployment, "alice@example.com")
                .await
                .expect("a user");
            let mut tx = deployment.begin().await.expect("a transaction");
            let token = create(&mut tx, alice.id).await.expect("a token");
            tx.commit().await.expect("committed");

            let in_a_year = now() + Duration::days(VALIDITY_DAYS + 1);
            let mut tx = deployment.begin().await.expect("a transaction");
            assert_eq!(delete_expired(&mut tx, now()).await.expect("none"), 0);
            assert_eq!(delete_expired(&mut tx, in_a_year).await.expect("one"), 1);
            tx.commit().await.expect("committed");
            assert!(users::get_by_token(&deployment, &token).await.is_err());
            assert_ne!(hash(&token), token);
        })
        .await;
    }
}
