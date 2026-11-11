//! Accounts: signing up, signing in and signing out. The server does
//! the work, and every sign-up and sign-in ends with a code emailed to
//! the person, see `docs/design/accounts-sign-in.md`. A sign-up writes
//! the person's account, `users`, unconfirmed, their user's row with
//! their name, `crate::users`, and their password hash; a sign-in
//! checks the password; both then make a pending sign-in, `tokens`,
//! and mail its six-digit code, `mails`, answering the pending token the
//! page holds. The code, `sign_in_with_code`, confirms the email the
//! first time and ends in `finish_in`: a session token for the browser's
//! cookie, or, for a sign-in a device started in the browser, a device
//! code the device exchanges for its API token,
//! `sign_in_with_device_code`, with the verifier only it holds. Every
//! way of proving who someone is, a provider's say-so one day, ends in
//! that same function. A sign-out deletes the token.
//!
//! A device never sees a password: it keeps the session the exchange
//! answered, `keep_signed_in`, as the `signed_in` device setting, and a
//! sign-out clears the setting and keeps the data, so signing in again
//! resumes from the cursor; which file a device signs into is the app's
//! business, see `docs/design/device-accounts.md`.

pub mod users;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::credentials::{self, CredentialError};
use crate::database::Transaction;
use crate::deployment::{Deployment, Role, now};
use crate::device_settings::{self, DeviceSettingsError, SIGNED_IN};
use crate::mailer::{MailError, Mailer};
use crate::mails;
use crate::sync::{SyncError, Transport};
use crate::tokens::{self, Context};
use crate::users::{SignedIn, User, UserError};

use self::users::{AccountUser, AccountUserError};

/// Wrong codes a pending sign-in takes before it is deleted.
pub const MAX_CODE_ATTEMPTS: i64 = 5;

/// What a finished sign-in answers: the user, their account's email,
/// which a device shows its accounts by, and the token that stands for
/// them from now on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub user: User,
    pub email: String,
    pub token: String,
}

/// What a sign-up or a sign-in answers: the pending token the page
/// sends back with the code that was emailed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeSent {
    pub pending: String,
}

/// What a sign-in a device started answers once the code matched: the
/// code the device exchanges for its session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceCode {
    pub code: String,
}

#[derive(Debug, Error)]
pub enum AccountError {
    #[error("only the server signs people up and in")]
    ServerOnly,
    #[error("only a device keeps someone signed in")]
    DeviceOnly,
    #[error("That email and password do not match")]
    InvalidEmailOrPassword,
    #[error("That code is wrong or has expired; sign in again to get a new one")]
    CodeDoesNotMatch,
    #[error("That code is wrong or has expired; sign in again")]
    UnknownDeviceCode,
    #[error(transparent)]
    User(#[from] UserError),
    #[error(transparent)]
    AccountUser(#[from] AccountUserError),
    #[error(transparent)]
    Credential(#[from] CredentialError),
    #[error("Could not send the email; try again")]
    Mail(#[from] MailError),
    #[error(transparent)]
    Sync(#[from] SyncError),
    #[error(transparent)]
    DeviceSettings(#[from] DeviceSettingsError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// What a code that matched ends in.
enum Finish<'a> {
    /// A token for the browser's cookie.
    Session,
    /// A code for the device whose challenge this is.
    Device { challenge: &'a str },
}

/// Registers the person on the server, unconfirmed, with their name
/// and password, and mails them a code: their account, their user's
/// row, their password and a pending sign-in, in one transaction, then
/// the email. An email that already has a confirmed account is answered
/// the same way, after the same work, and its owner is emailed that
/// they have one; an unconfirmed one gets this name and password and a
/// new code, so whoever reads the inbox gets the account.
pub async fn sign_up(
    deployment: &Deployment,
    mailer: &Mailer,
    email: &str,
    password: &str,
    name: &str,
) -> Result<CodeSent, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    crate::users::checked_name(name)?;
    let mut tx = deployment.begin().await?;
    let existing = users::get_by_email_in(&mut tx, email).await?;
    let (mail, pending) = match existing {
        Some(account) if account.confirmed_at.is_some() => {
            let _hashed_to_take_as_long = credentials::checked_hash(password).await?;
            let mail = mails::already_signed_up(&account.email);
            (mail, tokens::random_hex(32))
        }
        Some(account) => {
            crate::users::update_name_in(&mut tx, account.id, name).await?;
            credentials::set(&mut tx, account.id, password).await?;
            let (pending, code) = tokens::create_sign_in_code(&mut tx, account.id).await?;
            (mails::sign_in_code(&account.email, &code), pending)
        }
        None => {
            let account = users::create_in(&mut tx, Uuid::now_v7(), email, false).await?;
            crate::users::create_in(&mut tx, account.id, name).await?;
            credentials::set(&mut tx, account.id, password).await?;
            let (pending, code) = tokens::create_sign_in_code(&mut tx, account.id).await?;
            (mails::sign_in_code(&account.email, &code), pending)
        }
    };
    tx.commit().await?;
    mailer.send(mail).await?;
    Ok(CodeSent { pending })
}

/// Checks the password, on the server, and mails the person a code for
/// a pending sign-in. An unknown email is answered as a wrong password
/// is, after the same work.
pub async fn sign_in(
    deployment: &Deployment,
    mailer: &Mailer,
    email: &str,
    password: &str,
) -> Result<CodeSent, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let account = users::get_by_email_in(&mut tx, email).await?;
    let account_id = account.as_ref().map(|account| account.id);
    let matches = credentials::verify(&mut tx, account_id, password).await?;
    let Some(account) = account.filter(|_| matches) else {
        return Err(AccountError::InvalidEmailOrPassword);
    };
    let (pending, code) = tokens::create_sign_in_code(&mut tx, account.id).await?;
    tx.commit().await?;
    mailer
        .send(mails::sign_in_code(&account.email, &code))
        .await?;
    Ok(CodeSent { pending })
}

/// Mails the pending sign-in a new code, on the server, its ten minutes
/// starting over and its wrong attempts forgotten; the old code is no
/// longer good. For the "Send a new code" button; how often is the web
/// server's limit, see `docs/design/accounts-sign-in.md`.
pub async fn resend_code(
    deployment: &Deployment,
    mailer: &Mailer,
    pending: &str,
) -> Result<(), AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let row = tokens::get_valid(&mut tx, &tokens::hash(pending), Context::SignInCode, now())
        .await?
        .ok_or(AccountError::CodeDoesNotMatch)?;
    let account = live_account_in(&mut tx, row.user_id).await?;
    let code = tokens::reset_code(&mut tx, row.id).await?;
    tx.commit().await?;
    mailer
        .send(mails::sign_in_code(&account.email, &code))
        .await?;
    Ok(())
}

/// The address `resend_code` would mail, for the server to count the
/// email against before sending; the same refusal as `resend_code`.
pub async fn resend_code_to(
    deployment: &Deployment,
    pending: &str,
) -> Result<String, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let row = tokens::get_valid(&mut tx, &tokens::hash(pending), Context::SignInCode, now())
        .await?
        .ok_or(AccountError::CodeDoesNotMatch)?;
    let account = live_account_in(&mut tx, row.user_id).await?;
    tx.commit().await?;
    Ok(account.email)
}

/// Finishes the pending sign-in with the code that was emailed, on the
/// server, and makes a token for the browser's cookie. A wrong code
/// counts; after `MAX_CODE_ATTEMPTS` the pending sign-in is gone.
pub async fn sign_in_with_code(
    deployment: &Deployment,
    pending: &str,
    code: &str,
) -> Result<Session, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let (user, account) = match match_code_in(&mut tx, pending, code).await {
        Ok(matched) => matched,
        Err(error) => {
            tx.commit().await?;
            return Err(error);
        }
    };
    let token = finish_in(&mut tx, &user, Finish::Session).await?;
    tx.commit().await?;
    Ok(Session {
        user,
        email: account.email,
        token,
    })
}

/// Finishes the pending sign-in with the code, for a sign-in a device
/// started with the challenge: the device code it collects, and no
/// token for the browser.
pub async fn sign_in_with_code_for_device(
    deployment: &Deployment,
    pending: &str,
    code: &str,
    challenge: &str,
) -> Result<DeviceCode, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let (user, _account) = match match_code_in(&mut tx, pending, code).await {
        Ok(matched) => matched,
        Err(error) => {
            tx.commit().await?;
            return Err(error);
        }
    };
    let code = finish_in(&mut tx, &user, Finish::Device { challenge }).await?;
    tx.commit().await?;
    Ok(DeviceCode { code })
}

/// Exchanges the device code, with the verifier the device kept, for
/// the device's session, once: the row is found by the code and the
/// verifier's hash together, see `tokens::create_device_code`, and
/// deleted.
pub async fn sign_in_with_device_code(
    deployment: &Deployment,
    code: &str,
    verifier: &str,
) -> Result<Session, AccountError> {
    if deployment.role != Role::Server {
        return Err(AccountError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let token_hash = tokens::device_code_hash(code, &challenge_of(verifier));
    let row = tokens::get_valid(&mut tx, &token_hash, Context::DeviceCode, now())
        .await?
        .ok_or(AccountError::UnknownDeviceCode)?;
    tokens::delete_by_id(&mut tx, row.id).await?;
    let account = users::get_in(&mut tx, row.user_id)
        .await?
        .ok_or(AccountError::UnknownDeviceCode)?;
    let user = live_user_in(&mut tx, row.user_id)
        .await?
        .ok_or(AccountError::UnknownDeviceCode)?;
    let token = tokens::create(&mut tx, user.id, Context::Api).await?;
    tx.commit().await?;
    Ok(Session {
        user,
        email: account.email,
        token,
    })
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

/// What a device makes before it opens the browser: the verifier it
/// keeps to itself, and the challenge it puts in the sign-in URL, the
/// verifier's hash.
pub fn verifier_and_challenge() -> (String, String) {
    let verifier = tokens::random_hex(32);
    let challenge = challenge_of(&verifier);
    (verifier, challenge)
}

/// The challenge a verifier proves: its SHA-256 as hex.
pub fn challenge_of(verifier: &str) -> String {
    tokens::hash(verifier)
}

/// Exchanges the device code through the transport, `sign_in_with_device_code`
/// on the server: the session, kept nowhere yet.
pub async fn sign_in_with_device_code_through<T: Transport>(
    transport: &T,
    code: &str,
    verifier: &str,
) -> Result<Session, AccountError> {
    match transport.sign_in_with_device_code(code, verifier).await {
        Err(SyncError::Unauthorized) => Err(AccountError::UnknownDeviceCode),
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
    crate::users::upsert_in(&mut tx, &session.user).await?;
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
    let (_user, token) = crate::users::signed_in_with_token(deployment).await?;
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

/// The user of the pending sign-in and their account, once the code
/// matches: the pending sign-in is deleted and the email confirmed if
/// it was not yet. A wrong code is counted, and the one that reaches
/// `MAX_CODE_ATTEMPTS` deletes the pending sign-in; the caller commits
/// either way.
async fn match_code_in(
    tx: &mut Transaction,
    pending: &str,
    code: &str,
) -> Result<(User, AccountUser), AccountError> {
    let row = tokens::get_valid(tx, &tokens::hash(pending), Context::SignInCode, now())
        .await?
        .ok_or(AccountError::CodeDoesNotMatch)?;
    if row.code_hash.as_deref() != Some(tokens::hash(code.trim()).as_str()) {
        let attempts = tokens::count_attempt(tx, row.id).await?;
        if attempts >= MAX_CODE_ATTEMPTS {
            tokens::delete_by_id(tx, row.id).await?;
        }
        return Err(AccountError::CodeDoesNotMatch);
    }
    tokens::delete_by_id(tx, row.id).await?;
    let account = live_account_in(tx, row.user_id).await?;
    let user = live_user_in(tx, row.user_id)
        .await?
        .ok_or(AccountError::CodeDoesNotMatch)?;
    if account.confirmed_at.is_none() {
        users::confirm_in(tx, account.id).await?;
    }
    Ok((user, account))
}

/// Where every proven user ends: the browser's token, or the device
/// code for the device whose challenge was given.
async fn finish_in(
    tx: &mut Transaction,
    user: &User,
    finish: Finish<'_>,
) -> Result<String, AccountError> {
    Ok(match finish {
        Finish::Session => tokens::create(tx, user.id, Context::Session).await?,
        Finish::Device { challenge } => tokens::create_device_code(tx, user.id, challenge).await?,
    })
}

async fn live_account_in(tx: &mut Transaction, id: Uuid) -> Result<AccountUser, AccountError> {
    users::get_in(tx, id)
        .await?
        .ok_or(AccountError::CodeDoesNotMatch)
}

async fn live_user_in(tx: &mut Transaction, id: Uuid) -> Result<Option<User>, AccountError> {
    let user = crate::users::get_in(tx, id).await?;
    Ok(user.filter(|user| user.deleted_at.is_none()))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::database::with_each_database;
    use crate::sync::{self, InProcess};
    use crate::{posts, users};

    pub(crate) const PASSWORD: &str = "correct horse battery";

    /// The code the last email to the address carried.
    pub(crate) fn code_sent_to(mailer: &Mailer, email: &str) -> String {
        let mail = mailer.last_sent_to(email).expect("an email");
        mails::code_in(&mail).expect("a code in it")
    }

    /// A user signed up on the server and finished with their code, as
    /// a device: the session its exchange answered, with an API token.
    pub(crate) async fn signed_up_for_a_device(server: &Deployment, email: &str) -> Session {
        let mailer = Mailer::memory();
        let sent = sign_up(server, &mailer, email, PASSWORD, "Alice")
            .await
            .expect("signed up");
        let code = code_sent_to(&mailer, email);
        let (verifier, challenge) = verifier_and_challenge();
        let device_code = sign_in_with_code_for_device(server, &sent.pending, &code, &challenge)
            .await
            .expect("the code matched");
        sign_in_with_device_code(server, &device_code.code, &verifier)
            .await
            .expect("exchanged")
    }

    #[tokio::test]
    async fn accounts_sign_up_mails_a_code_that_confirms_and_signs_in_then_sign_in_and_out() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let sent = sign_up(&server, &mailer, " Alice@Example.com ", PASSWORD, "Alice")
                .await
                .expect("signed up");
            assert_eq!(sent.pending.len(), 64);
            let mail = mailer.last_sent_to("alice@example.com").expect("mailed");
            let code = mails::code_in(&mail).expect("a code");
            assert!(mail.subject.contains("code"));

            let error = sign_in_with_code(&server, &sent.pending, "000000")
                .await
                .expect_err("wrong, unless the code is 000000");
            assert!(matches!(error, AccountError::CodeDoesNotMatch) || code == "000000");

            let signed_up = sign_in_with_code(&server, &sent.pending, &format!(" {code} "))
                .await
                .expect("the code matched, trimmed");
            assert_eq!(signed_up.email, "alice@example.com");
            assert_eq!(signed_up.user.name, "Alice");
            let account = super::users::get(&server, signed_up.user.id)
                .await
                .expect("their account");
            assert!(account.confirmed_at.is_some(), "the email is proved");
            assert_eq!(
                users::get_by_token(&server, &signed_up.token)
                    .await
                    .expect("stands for alice"),
                signed_up.user
            );
            assert!(
                matches!(
                    sign_in_with_code(&server, &sent.pending, &code).await,
                    Err(AccountError::CodeDoesNotMatch)
                ),
                "a code is used once"
            );

            let sent = sign_in(&server, &mailer, "ALICE@example.com", PASSWORD)
                .await
                .expect("signed in");
            let code = code_sent_to(&mailer, "alice@example.com");
            let signed_in = sign_in_with_code(&server, &sent.pending, &code)
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
    async fn accounts_sign_up_refuses_a_bad_email_or_password_and_answers_a_taken_email_quietly() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let error = sign_up(&server, &mailer, "alice", PASSWORD, "Alice")
                .await
                .expect_err("no @");
            assert!(matches!(
                error,
                AccountError::AccountUser(AccountUserError::InvalidEmail)
            ));
            let error = sign_up(&server, &mailer, "alice@example.com", "short", "Alice")
                .await
                .expect_err("short");
            assert!(matches!(
                error,
                AccountError::Credential(CredentialError::PasswordTooShort)
            ));
            assert!(
                sign_in(&server, &mailer, "alice@example.com", PASSWORD)
                    .await
                    .is_err(),
                "a refused sign-up wrote nothing"
            );
            assert!(mailer.sent().is_empty());

            let first = sign_up(&server, &mailer, "alice@example.com", PASSWORD, "Alice")
                .await
                .expect("signed up");
            let first_code = code_sent_to(&mailer, "alice@example.com");
            let again = sign_up(
                &server,
                &mailer,
                "alice@example.com",
                "another horse battery",
                "Alice Again",
            )
            .await
            .expect("signed up again before confirming");
            assert!(
                matches!(
                    sign_in_with_code(&server, &first.pending, &first_code).await,
                    Err(AccountError::CodeDoesNotMatch)
                ),
                "the first pending sign-in was replaced"
            );
            let code = code_sent_to(&mailer, "alice@example.com");
            let session = sign_in_with_code(&server, &again.pending, &code)
                .await
                .expect("the second one works");
            assert_eq!(session.user.name, "Alice Again", "with the second name");
            assert!(
                sign_in(
                    &server,
                    &mailer,
                    "alice@example.com",
                    "another horse battery"
                )
                .await
                .is_ok(),
                "with the second password"
            );
            let error = sign_up(&server, &mailer, "alice@example.com", "short", "Alice")
                .await
                .expect_err("a bad password is refused before anything else");
            assert!(matches!(
                error,
                AccountError::Credential(CredentialError::PasswordTooShort)
            ));

            let sent_before = mailer.sent().len();
            let taken = sign_up(&server, &mailer, "alice@example.com", PASSWORD, "Alice")
                .await
                .expect("answered as a new one");
            assert_eq!(taken.pending.len(), 64);
            let mail = mailer.last_sent_to("alice@example.com").expect("mailed");
            assert_eq!(mailer.sent().len(), sent_before + 1);
            assert_eq!(mail.subject, "You already have an account");
            assert!(matches!(
                sign_in_with_code(&server, &taken.pending, "123456").await,
                Err(AccountError::CodeDoesNotMatch)
            ));
            assert!(
                sign_in(
                    &server,
                    &mailer,
                    "alice@example.com",
                    "another horse battery"
                )
                .await
                .is_ok(),
                "the password was not touched"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_sign_in_refuses_a_wrong_password_and_an_unknown_email_alike() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            signed_up_for_a_device(&server, "alice@example.com").await;
            let error = sign_in(&server, &mailer, "alice@example.com", "wrong horse battery")
                .await
                .expect_err("wrong password");
            assert!(matches!(error, AccountError::InvalidEmailOrPassword));
            let error = sign_in(&server, &mailer, "nobody@example.com", PASSWORD)
                .await
                .expect_err("unknown email");
            assert!(matches!(error, AccountError::InvalidEmailOrPassword));
            assert!(mailer.sent().is_empty());
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_a_code_takes_five_wrong_tries_then_the_sign_in_is_gone() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let sent = sign_up(&server, &mailer, "alice@example.com", PASSWORD, "Alice")
                .await
                .expect("signed up");
            let code = code_sent_to(&mailer, "alice@example.com");
            let wrong = if code == "999999" { "999998" } else { "999999" };
            for _ in 0..MAX_CODE_ATTEMPTS {
                assert!(matches!(
                    sign_in_with_code(&server, &sent.pending, wrong).await,
                    Err(AccountError::CodeDoesNotMatch)
                ));
            }
            assert!(
                matches!(
                    sign_in_with_code(&server, &sent.pending, &code).await,
                    Err(AccountError::CodeDoesNotMatch)
                ),
                "the right code is too late"
            );
            let sent = sign_in(&server, &mailer, "alice@example.com", PASSWORD)
                .await
                .expect("signed in again");
            let code = code_sent_to(&mailer, "alice@example.com");
            let session = sign_in_with_code(&server, &sent.pending, &code)
                .await
                .expect("a fresh code works");
            assert_eq!(session.email, "alice@example.com");
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_a_new_code_replaces_the_old_one_for_the_same_pending_sign_in() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let sent = sign_up(&server, &mailer, "alice@example.com", PASSWORD, "Alice")
                .await
                .expect("signed up");
            let first = code_sent_to(&mailer, "alice@example.com");
            resend_code(&server, &mailer, &sent.pending)
                .await
                .expect("a new code sent");
            let second = code_sent_to(&mailer, "alice@example.com");
            assert_eq!(mailer.sent().len(), 2);
            assert_eq!(
                resend_code_to(&server, &sent.pending)
                    .await
                    .expect("the address"),
                "alice@example.com"
            );
            if first != second {
                assert!(
                    matches!(
                        sign_in_with_code(&server, &sent.pending, &first).await,
                        Err(AccountError::CodeDoesNotMatch)
                    ),
                    "the old code is no longer good"
                );
            }
            sign_in_with_code(&server, &sent.pending, &second)
                .await
                .expect("the new one is");
            assert!(
                matches!(
                    resend_code(&server, &mailer, &sent.pending).await,
                    Err(AccountError::CodeDoesNotMatch)
                ),
                "nothing pending any more"
            );
            assert!(matches!(
                resend_code(&server, &mailer, "not a pending token").await,
                Err(AccountError::CodeDoesNotMatch)
            ));
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_a_device_code_needs_the_verifier_and_works_once() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let mailer = Mailer::memory();
            let sent = sign_up(&server, &mailer, "alice@example.com", PASSWORD, "Alice")
                .await
                .expect("signed up");
            let code = code_sent_to(&mailer, "alice@example.com");
            let (verifier, challenge) = verifier_and_challenge();
            assert_eq!(challenge, challenge_of(&verifier));
            let device_code =
                sign_in_with_code_for_device(&server, &sent.pending, &code, &challenge)
                    .await
                    .expect("the code matched");
            assert_eq!(device_code.code.len(), 32);

            let error = sign_in_with_device_code(&server, &device_code.code, "another verifier")
                .await
                .expect_err("not without the verifier");
            assert!(matches!(error, AccountError::UnknownDeviceCode));
            let session = sign_in_with_device_code(&server, &device_code.code, &verifier)
                .await
                .expect("exchanged");
            assert_eq!(session.email, "alice@example.com");
            assert_eq!(
                users::get_by_token(&server, &session.token)
                    .await
                    .expect("an API token"),
                session.user
            );
            let error = sign_in_with_device_code(&server, &device_code.code, &verifier)
                .await
                .expect_err("once");
            assert!(matches!(error, AccountError::UnknownDeviceCode));
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_are_the_servers_alone() {
        let device = users::tests::device().await;
        let mailer = Mailer::memory();
        assert!(matches!(
            sign_up(&device, &mailer, "alice@example.com", PASSWORD, "Alice").await,
            Err(AccountError::ServerOnly)
        ));
        assert!(matches!(
            sign_in(&device, &mailer, "alice@example.com", PASSWORD).await,
            Err(AccountError::ServerOnly)
        ));
        assert!(matches!(
            sign_in_with_code(&device, "pending", "123456").await,
            Err(AccountError::ServerOnly)
        ));
        assert!(matches!(
            sign_in_with_device_code(&device, "code", "verifier").await,
            Err(AccountError::ServerOnly)
        ));
        assert!(matches!(
            sign_out(&device, "a token").await,
            Err(AccountError::ServerOnly)
        ));
    }

    #[tokio::test]
    async fn accounts_a_device_signs_in_syncs_signs_out_and_resumes_on_the_next_sign_in() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let transport = InProcess { server: &server };
            let device = users::tests::device().await;
            assert!(matches!(
                sign_in_with_device_code_through(&transport, "no code", "no verifier").await,
                Err(AccountError::UnknownDeviceCode)
            ));

            let session = signed_up_for_a_device(&server, "alice@example.com").await;
            let alice = keep_signed_in(&device, session).await.expect("kept");
            assert_eq!(users::signed_in(&device).await.expect("alice"), alice);

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

            let mailer = Mailer::memory();
            let sent = sign_in(&server, &mailer, "alice@example.com", PASSWORD)
                .await
                .expect("signed in on the server");
            let code = code_sent_to(&mailer, "alice@example.com");
            let (verifier, challenge) = verifier_and_challenge();
            let device_code =
                sign_in_with_code_for_device(&server, &sent.pending, &code, &challenge)
                    .await
                    .expect("the code matched");
            let session =
                sign_in_with_device_code_through(&transport, &device_code.code, &verifier)
                    .await
                    .expect("exchanged through the transport");
            let again = keep_signed_in(&device, session).await.expect("kept again");
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
            let session = signed_up_for_a_device(&server, "alice@example.com").await;
            let alice = keep_signed_in(&device, session).await.expect("kept");
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

            let mailer = Mailer::memory();
            let sent = sign_in(&server, &mailer, "alice@example.com", PASSWORD)
                .await
                .expect("signed in on the server");
            let code = code_sent_to(&mailer, "alice@example.com");
            let (verifier, challenge) = verifier_and_challenge();
            let device_code =
                sign_in_with_code_for_device(&server, &sent.pending, &code, &challenge)
                    .await
                    .expect("the code matched");
            let session =
                sign_in_with_device_code_through(&transport, &device_code.code, &verifier)
                    .await
                    .expect("exchanged");
            keep_signed_in(&device, session)
                .await
                .expect("signed in again");
            let report = sync::run(&device, &transport).await.expect("synced");
            assert_eq!(report.pushed, 1);
            assert!(posts::get(&server, &alice, post.id).await.is_ok());
        })
        .await;
    }

    #[tokio::test]
    async fn accounts_a_401_is_a_sign_out_done_elsewhere_and_the_data_stays() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let transport = InProcess { server: &server };
            let session = signed_up_for_a_device(&server, "alice@example.com").await;
            let device = users::tests::device().await;
            assert!(matches!(
                users::signed_in(&device).await,
                Err(UserError::NotSignedIn)
            ));
            let alice = keep_signed_in(&device, session.clone())
                .await
                .expect("kept");
            assert_eq!(users::signed_in(&device).await.expect("alice"), alice);
            posts::create(&device, &alice, "kept through it", &[])
                .await
                .expect("a post");

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
            assert_eq!(sync::status(&device).await.expect("status").pending, 1);
        })
        .await;
    }
}
