//! Accounts: signing up, signing in and signing out with an email and a
//! password. The server does the work: a sign-up writes the user's row,
//! their password hash and a token in one transaction, a sign-in checks
//! the password and makes a token, a sign-out deletes the token. A
//! device asks the server through the sync transport, keeps who signed
//! in and their token as the `signed_in` device setting, and drops its
//! copy of the data at a sign-out, so the next person starts from
//! nothing. The web server keeps a browser signed in with the same token
//! in a cookie, see `axum/src/current_user.rs`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::credentials::{self, CredentialError};
use crate::database::sql;
use crate::deployment::{Deployment, Role};
use crate::device_settings::{self, DeviceSettingsError, SIGNED_IN};
use crate::sync::{self, SyncError, Transport};
use crate::tokens;
use crate::users::{self, SignedIn, User, UserError};

/// What a sign-up or a sign-in answers: the user, and the token that
/// stands for them from now on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub user: User,
    pub token: String,
}

/// What a sign-out on a device answers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum SignOut {
    /// Nobody is signed in on the device any more.
    SignedOut,
    /// Nothing was done: these changes have not reached the server, or
    /// were not kept by it, and the sign-out would lose them. The app
    /// asks the person, who signs out losing them or keeps them.
    ChangesWouldBeLost { pending: i64, conflicts: i64 },
}

#[derive(Debug, Error)]
pub enum AccountError {
    #[error("only the server signs people up and in")]
    ServerOnly,
    #[error("only a device keeps someone signed in")]
    DeviceOnly,
    #[error("That email and password do not match")]
    InvalidEmailOrPassword,
    #[error("Someone is signed in on this device; sign out first")]
    AlreadySignedIn,
    #[error(transparent)]
    User(#[from] UserError),
    #[error(transparent)]
    Credential(#[from] CredentialError),
    #[error(transparent)]
    Sync(#[from] SyncError),
    #[error(transparent)]
    DeviceSettings(#[from] DeviceSettingsError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Registers the user on the server and signs them in: their row, their
/// password and a token, in one transaction.
pub async fn sign_up(
    deployment: &Deployment,
    email: &str,
    password: &str,
) -> Result<Session, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let user = users::create_in(&mut tx, email).await?;
    credentials::create(&mut tx, user.id, password).await?;
    let token = tokens::create(&mut tx, user.id).await?;
    tx.commit().await?;
    Ok(Session { user, token })
}

/// Checks the password and makes a token, on the server. An unknown
/// email is answered as a wrong password is, after the same work.
pub async fn sign_in(
    deployment: &Deployment,
    email: &str,
    password: &str,
) -> Result<Session, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let user = users::get_by_email_in(&mut tx, email).await?;
    let user_id = user.as_ref().map(|user| user.id);
    let matches = credentials::verify(&mut tx, user_id, password).await?;
    let Some(user) = user.filter(|_| matches) else {
        return Err(AccountError::InvalidEmailOrPassword);
    };
    let token = tokens::create(&mut tx, user.id).await?;
    tx.commit().await?;
    Ok(Session { user, token })
}

/// Deletes the token, on the server, so it stands for nobody from now
/// on; one the server does not know is already gone.
pub async fn sign_out(deployment: &Deployment, token: &str) -> Result<(), AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    tokens::delete(&mut tx, token).await?;
    tx.commit().await?;
    Ok(())
}

/// Signs up on the server through the transport, then signs in on this
/// device, see `sign_in_on_device`.
pub async fn sign_up_on_device<T: Transport>(
    deployment: &Deployment,
    transport: &T,
    email: &str,
    password: &str,
) -> Result<User, AccountError> {
    nobody_signed_in(deployment).await?;
    let session = transport.sign_up(email, password).await?;
    keep_signed_in(deployment, session).await
}

/// Signs in on the server through the transport and keeps the answer on
/// this device: the user's row, and the setting naming them with their
/// token. Whatever the device held before goes, so the first sync pulls
/// the user's own data from nothing.
pub async fn sign_in_on_device<T: Transport>(
    deployment: &Deployment,
    transport: &T,
    email: &str,
    password: &str,
) -> Result<User, AccountError> {
    nobody_signed_in(deployment).await?;
    let session = match transport.sign_in(email, password).await {
        Err(SyncError::Unauthorized) => return Err(AccountError::InvalidEmailOrPassword),
        answer => answer?,
    };
    keep_signed_in(deployment, session).await
}

/// Tells the server to forget the device's token, then drops the
/// device's copy of the data and the setting, so nobody is signed in.
/// While a change has not reached the server, or was not kept by it,
/// nothing is done and the counts are answered instead, unless the
/// person chose to sign out `losing_changes`.
pub async fn sign_out_on_device<T: Transport>(
    deployment: &Deployment,
    transport: &T,
    losing_changes: bool,
) -> Result<SignOut, AccountError> {
    if deployment.role != Role::Device {
        return Err(AccountError::DeviceOnly);
    }
    let (_user, token) = users::signed_in_with_token(deployment).await?;
    if !losing_changes {
        let status = sync::status(deployment).await?;
        if status.pending > 0 || status.conflicts > 0 {
            return Ok(SignOut::ChangesWouldBeLost {
                pending: status.pending,
                conflicts: status.conflicts,
            });
        }
    }
    match transport.sign_out(&token).await {
        Ok(()) | Err(SyncError::Unauthorized) => {}
        Err(error) => return Err(error.into()),
    }
    let mut tx = deployment.begin().await?;
    sql!(&mut tx, sync_state::wipe())?;
    device_settings::set(&mut tx, SIGNED_IN, &None::<SignedIn>).await?;
    tx.commit().await?;
    Ok(SignOut::SignedOut)
}

async fn nobody_signed_in(deployment: &Deployment) -> Result<(), AccountError> {
    if deployment.role != Role::Device {
        return Err(AccountError::DeviceOnly);
    }
    match users::signed_in_with_token(deployment).await {
        Err(UserError::NotSignedIn) => Ok(()),
        Ok(_) => Err(AccountError::AlreadySignedIn),
        Err(error) => Err(error.into()),
    }
}

async fn keep_signed_in(deployment: &Deployment, session: Session) -> Result<User, AccountError> {
    let mut tx = deployment.begin().await?;
    sql!(&mut tx, sync_state::wipe())?;
    users::upsert_in(&mut tx, &session.user).await?;
    let signed_in = SignedIn {
        user_id: session.user.id,
        token: session.token,
    };
    device_settings::set(&mut tx, SIGNED_IN, &Some(signed_in)).await?;
    tx.commit().await?;
    Ok(session.user)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::with_each_database;
    use crate::sync::InProcess;
    use crate::{posts, users};

    const PASSWORD: &str = "correct horse battery";

    #[tokio::test]
    async fn accounts_sign_up_then_sign_in_and_out() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let signed_up = sign_up(&server, "alice@example.com", PASSWORD)
                .await
                .expect("signed up");
            assert_eq!(signed_up.user.email, "alice@example.com");
            assert_eq!(
                users::get_by_token(&server, &signed_up.token)
                    .await
                    .expect("stands for alice"),
                signed_up.user
            );

            let signed_in = sign_in(&server, " alice@example.com ", PASSWORD)
                .await
                .expect("signed in");
            assert_eq!(signed_in.user, signed_up.user);
            assert_ne!(signed_in.token, signed_up.token, "a token of its own");

            sign_out(&server, &signed_in.token)
                .await
                .expect("signed out");
            assert!(matches!(
                users::get_by_token(&server, &signed_in.token).await,
                Err(UserError::UnknownToken)
            ));
            assert!(
                users::get_by_token(&server, &signed_up.token).await.is_ok(),
                "the other session stays"
            );
            sign_out(&server, &signed_in.token)
                .await
                .expect("already gone is fine");
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_sign_up_refuses_a_bad_email_or_password_or_a_taken_email() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let error = sign_up(&server, "alice", PASSWORD).await.expect_err("no @");
            assert!(matches!(error, AccountError::User(UserError::InvalidEmail)));
            let error = sign_up(&server, "alice@example.com", "short")
                .await
                .expect_err("short");
            assert!(matches!(
                error,
                AccountError::Credential(CredentialError::PasswordTooShort)
            ));
            assert!(
                sign_in(&server, "alice@example.com", PASSWORD)
                    .await
                    .is_err(),
                "a refused sign-up wrote nothing"
            );

            sign_up(&server, "alice@example.com", PASSWORD)
                .await
                .expect("signed up");
            let error = sign_up(&server, "alice@example.com", PASSWORD)
                .await
                .expect_err("taken");
            assert!(matches!(error, AccountError::User(UserError::EmailTaken)));
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_sign_in_refuses_a_wrong_password_and_an_unknown_email_alike() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            sign_up(&server, "alice@example.com", PASSWORD)
                .await
                .expect("signed up");
            let error = sign_in(&server, "alice@example.com", "wrong horse battery")
                .await
                .expect_err("wrong password");
            assert!(matches!(error, AccountError::InvalidEmailOrPassword));
            let error = sign_in(&server, "nobody@example.com", PASSWORD)
                .await
                .expect_err("unknown email");
            assert!(matches!(error, AccountError::InvalidEmailOrPassword));
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_are_the_servers_alone() {
        let device = users::tests::device().await;
        assert!(matches!(
            sign_up(&device, "alice@example.com", PASSWORD).await,
            Err(AccountError::ServerOnly)
        ));
        assert!(matches!(
            sign_in(&device, "alice@example.com", PASSWORD).await,
            Err(AccountError::ServerOnly)
        ));
        assert!(matches!(
            sign_out(&device, "a token").await,
            Err(AccountError::ServerOnly)
        ));
    }

    #[tokio::test]
    async fn accounts_a_device_signs_up_syncs_and_signs_out_from_nothing_to_nothing() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let transport = InProcess { server: &server };
            let device = users::tests::device().await;
            assert!(matches!(
                sign_in_on_device(&device, &transport, "alice@example.com", PASSWORD).await,
                Err(AccountError::InvalidEmailOrPassword)
            ));

            let alice = sign_up_on_device(&device, &transport, "alice@example.com", PASSWORD)
                .await
                .expect("signed up");
            assert_eq!(users::signed_in(&device).await.expect("alice"), alice);
            assert!(matches!(
                sign_in_on_device(&device, &transport, "alice@example.com", PASSWORD).await,
                Err(AccountError::AlreadySignedIn)
            ));

            let post = posts::create(&device, &alice, "from the device", &[])
                .await
                .expect("a post");
            assert_eq!(
                sign_out_on_device(&device, &transport, false)
                    .await
                    .expect("asked"),
                SignOut::ChangesWouldBeLost {
                    pending: 1,
                    conflicts: 0
                },
                "a change waits, so nothing was done"
            );
            assert_eq!(users::signed_in(&device).await.expect("still alice"), alice);
            sync::run(&device, &transport).await.expect("synced");
            assert_eq!(
                posts::get(&server, &alice, post.id)
                    .await
                    .expect("on the server")
                    .post
                    .body,
                "from the device"
            );

            let (_, token) = users::signed_in_with_token(&device)
                .await
                .expect("the token");
            assert_eq!(
                sign_out_on_device(&device, &transport, false)
                    .await
                    .expect("signed out"),
                SignOut::SignedOut
            );
            assert!(matches!(
                users::signed_in(&device).await,
                Err(UserError::NotSignedIn)
            ));
            assert!(matches!(
                users::get_by_token(&server, &token).await,
                Err(UserError::UnknownToken)
            ));
            let mut tx = device.begin().await.expect("a transaction");
            assert!(
                posts::get_in(&mut tx, post.id)
                    .await
                    .expect("looked")
                    .is_none(),
                "the device's copy is gone"
            );
            assert!(
                users::get_in(&mut tx, alice.id)
                    .await
                    .expect("looked")
                    .is_none()
            );
            tx.commit().await.expect("committed");

            let again = sign_in_on_device(&device, &transport, "alice@example.com", PASSWORD)
                .await
                .expect("signed in again");
            assert_eq!(again, alice);
            let report = sync::run(&device, &transport).await.expect("synced");
            assert_eq!(report.pulled, 2, "her row and her post");
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_a_device_signs_out_losing_changes_when_the_person_chose_to() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let transport = InProcess { server: &server };
            let device = users::tests::device().await;
            let alice = sign_up_on_device(&device, &transport, "alice@example.com", PASSWORD)
                .await
                .expect("signed up");
            let post = posts::create(&device, &alice, "never synced", &[])
                .await
                .expect("a post");
            assert_eq!(
                sign_out_on_device(&device, &transport, true)
                    .await
                    .expect("signed out"),
                SignOut::SignedOut
            );
            assert!(matches!(
                users::signed_in(&device).await,
                Err(UserError::NotSignedIn)
            ));
            assert!(
                posts::get(&server, &alice, post.id).await.is_err(),
                "the change never reached the server"
            );
            let status = sync::status(&device).await.expect("the status");
            assert_eq!((status.pending, status.conflicts), (0, 0), "and is gone");
        })
        .await;
    }
}
