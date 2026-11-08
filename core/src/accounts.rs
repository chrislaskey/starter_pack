//! Accounts: signing up, signing in and signing out with an email and a
//! password. The server does the work: a sign-up writes the user's row,
//! their password hash and a token in one transaction, a sign-in checks
//! the password and makes a token, a sign-out deletes the token. A
//! device asks the server through the sync transport and keeps who
//! signed in and their token as the `signed_in` device setting. A
//! sign-out clears the setting and keeps the data, so signing in again
//! resumes from the cursor; which file a device signs into, and when a
//! file is deleted, is the app's business, see
//! `docs/design/device-accounts.md`. The web server keeps a browser
//! signed in with the same token in a cookie, see
//! `axum/src/current_user.rs`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::credentials::{self, CredentialError};
use crate::deployment::{Deployment, Role};
use crate::device_settings::{self, DeviceSettingsError, SIGNED_IN};
use crate::sync::{SyncError, Transport};
use crate::tokens;
use crate::users::{self, SignedIn, User, UserError};

/// What a sign-up or a sign-in answers: the user, and the token that
/// stands for them from now on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub user: User,
    pub token: String,
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

/// Signs up on the server through the transport, `sign_up_through`,
/// then keeps the answer on this device, `keep_signed_in`.
pub async fn sign_up_on_device<T: Transport>(
    deployment: &Deployment,
    transport: &T,
    email: &str,
    password: &str,
) -> Result<User, AccountError> {
    nobody_signed_in(deployment).await?;
    let session = sign_up_through(transport, email, password).await?;
    keep_signed_in(deployment, session).await
}

/// Signs in on the server through the transport, `sign_in_through`,
/// then keeps the answer on this device, `keep_signed_in`. The two
/// halves are callable apart, for an app that chooses the device
/// database by the user the server answered.
pub async fn sign_in_on_device<T: Transport>(
    deployment: &Deployment,
    transport: &T,
    email: &str,
    password: &str,
) -> Result<User, AccountError> {
    nobody_signed_in(deployment).await?;
    let session = sign_in_through(transport, email, password).await?;
    keep_signed_in(deployment, session).await
}

/// Asks the server, through the transport, to register the user and
/// sign them in: the user and a token, kept nowhere yet.
pub async fn sign_up_through<T: Transport>(
    transport: &T,
    email: &str,
    password: &str,
) -> Result<Session, AccountError> {
    Ok(transport.sign_up(email, password).await?)
}

/// Asks the server, through the transport, to sign the user in: the
/// user and a token, kept nowhere yet.
pub async fn sign_in_through<T: Transport>(
    transport: &T,
    email: &str,
    password: &str,
) -> Result<Session, AccountError> {
    match transport.sign_in(email, password).await {
        Err(SyncError::Unauthorized) => Err(AccountError::InvalidEmailOrPassword),
        answer => Ok(answer?),
    }
}

/// Keeps a sign-in on this device: the user's row, and the setting
/// naming them with their token. Whatever the device holds stays, so a
/// sync resumes from its cursor; the app hands a user their own
/// database, never another's.
pub async fn keep_signed_in(
    deployment: &Deployment,
    session: Session,
) -> Result<User, AccountError> {
    if deployment.role != Role::Device {
        return Err(AccountError::DeviceOnly);
    }
    let mut tx = deployment.begin().await?;
    users::upsert_in(&mut tx, &session.user).await?;
    let signed_in = SignedIn {
        user_id: session.user.id,
        token: session.token,
    };
    device_settings::set(&mut tx, SIGNED_IN, &Some(signed_in)).await?;
    tx.commit().await?;
    Ok(session.user)
}

/// Tells the server to forget the device's token and clears the setting,
/// so nobody is signed in. The data stays, pending and conflicted
/// changes included, for the next sign-in to resume from; a token the
/// server no longer knows is already forgotten.
pub async fn sign_out_on_device<T: Transport>(
    deployment: &Deployment,
    transport: &T,
) -> Result<(), AccountError> {
    if deployment.role != Role::Device {
        return Err(AccountError::DeviceOnly);
    }
    let (_user, token) = users::signed_in_with_token(deployment).await?;
    match transport.sign_out(&token).await {
        Ok(()) | Err(SyncError::Unauthorized) => {}
        Err(error) => return Err(error.into()),
    }
    forget_signed_in(deployment).await
}

/// Clears the setting without telling the server, when the server has
/// already forgotten the token: it answered 401. The data stays.
pub async fn forget_signed_in(deployment: &Deployment) -> Result<(), AccountError> {
    if deployment.role != Role::Device {
        return Err(AccountError::DeviceOnly);
    }
    let mut tx = deployment.begin().await?;
    device_settings::set(&mut tx, SIGNED_IN, &None::<SignedIn>).await?;
    tx.commit().await?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::with_each_database;
    use crate::sync::{self, InProcess};
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
    async fn accounts_a_device_signs_up_syncs_signs_out_and_resumes_on_the_next_sign_in() {
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
            sync::run(&device, &transport).await.expect("synced");
            let cursor = sync::status(&device).await.expect("status").cursor;
            assert!(cursor.is_some(), "the first pull set the cursor");
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
            sign_out_on_device(&device, &transport)
                .await
                .expect("signed out");
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
                    .is_some(),
                "the device's copy stays"
            );
            tx.commit().await.expect("committed");
            assert!(
                sync::run(&device, &transport)
                    .await
                    .expect("ran")
                    .signed_out,
                "and nothing syncs meanwhile"
            );

            let again = sign_in_on_device(&device, &transport, "alice@example.com", PASSWORD)
                .await
                .expect("signed in again");
            assert_eq!(again, alice);
            assert_eq!(
                sync::status(&device).await.expect("status").cursor,
                cursor,
                "the cursor was kept through the sign-out, so the sync resumes"
            );
            let report = sync::run(&device, &transport).await.expect("synced");
            assert!(!report.resynced);
            assert_eq!(
                posts::list(&device, &alice).await.expect("her posts").len(),
                1
            );
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_a_sign_out_keeps_a_pending_change_for_the_next_sign_in_to_push() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let transport = InProcess { server: &server };
            let device = users::tests::device().await;
            let alice = sign_up_on_device(&device, &transport, "alice@example.com", PASSWORD)
                .await
                .expect("signed up");
            let post = posts::create(&device, &alice, "written, then signed out", &[])
                .await
                .expect("a post");
            sign_out_on_device(&device, &transport)
                .await
                .expect("signed out");
            assert!(
                posts::get(&server, &alice, post.id).await.is_err(),
                "not on the server yet"
            );
            assert_eq!(sync::status(&device).await.expect("status").pending, 1);

            sign_in_on_device(&device, &transport, "alice@example.com", PASSWORD)
                .await
                .expect("signed in again");
            let report = sync::run(&device, &transport).await.expect("synced");
            assert_eq!(report.pushed, 1);
            assert!(posts::get(&server, &alice, post.id).await.is_ok());
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_the_two_halves_of_a_sign_in_and_a_401_forgotten() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let transport = InProcess { server: &server };
            let session = sign_up_through(&transport, "alice@example.com", PASSWORD)
                .await
                .expect("signed up on the server");
            let device = users::tests::device().await;
            assert!(matches!(
                users::signed_in(&device).await,
                Err(UserError::NotSignedIn)
            ));
            let alice = keep_signed_in(&device, session.clone())
                .await
                .expect("kept");
            assert_eq!(users::signed_in(&device).await.expect("alice"), alice);

            sign_out(&server, &session.token)
                .await
                .expect("the token deleted elsewhere");
            let error = sync::run(&device, &transport)
                .await
                .expect_err("the server no longer knows the token");
            assert!(matches!(error, SyncError::Unauthorized));
            forget_signed_in(&device).await.expect("forgotten");
            assert!(matches!(
                users::signed_in(&device).await,
                Err(UserError::NotSignedIn)
            ));
            let session = sign_in_through(&transport, "alice@example.com", PASSWORD)
                .await
                .expect("signed in on the server");
            assert_eq!(session.user, alice);
            let error = sign_in_through(&transport, "alice@example.com", "wrong horse battery")
                .await
                .expect_err("wrong");
            assert!(matches!(error, AccountError::InvalidEmailOrPassword));
        })
        .await;
    }
}
