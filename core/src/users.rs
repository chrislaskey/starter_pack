//! Users: the app's view of each account, one row per person, written
//! by the server at sign-up and synced to devices like any row, so a
//! post can show who wrote it. Credentials never leave the server: the
//! password is in `credentials`, the tokens in `tokens`, both with the
//! user's id, and the flows are in `accounts`. Every core function takes
//! the `User` acting: the server resolves it from the request's token,
//! `get_by_token`, and a device keeps who is signed in as a device
//! setting, `signed_in`. Nobody acts until someone signs in.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

use crate::database::{Transaction, sql};
use crate::deployment::{Deployment, Role, now};
use crate::device_settings::{self, DeviceSettingsError, SIGNED_IN};
use crate::events::{self, Answer, Conflicts, Event, Fields};
use crate::sync::{Cursor, TableRow};
use crate::tokens;

pub const TABLE: &str = "app_users";

/// Phoenix's rule.
pub const MAX_EMAIL_LENGTH: usize = 160;

/// A user as the pages show one, with the sync columns every synced row
/// carries, see `docs/design/offline-sync.md`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: i64,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_id: Option<Uuid>,
    /// When the user first proved their email with a code sent to it;
    /// none until then, and such a user has no token, see `accounts`.
    #[serde(default)]
    pub confirmed_at: Option<DateTime<Utc>>,
    /// The public URL of their picture in object storage, see `avatars`.
    #[serde(default)]
    pub avatar_url: Option<String>,
}

/// Who is signed in on a device, and the token the device sends the
/// server for them: the `signed_in` device setting, written by a
/// sign-in and cleared by a sign-out, see `accounts`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedIn {
    pub user_id: Uuid,
    pub token: String,
}

#[derive(Debug, Error)]
pub enum UserError {
    #[error("user {id} not found")]
    NotFound { id: Uuid },
    #[error("an email needs an @ and no spaces, {MAX_EMAIL_LENGTH} characters at most")]
    InvalidEmail,
    #[error("that email already has an account")]
    EmailTaken,
    #[error("only the server creates users")]
    ServerOnly,
    #[error("The server did not accept this token")]
    UnknownToken,
    #[error("Nobody is signed in")]
    NotSignedIn,
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    DeviceSettings(#[from] DeviceSettingsError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<serde_json::Error> for UserError {
    fn from(error: serde_json::Error) -> UserError {
        UserError::Invalid(error.to_string())
    }
}

/// One user by their id, unless deleted.
pub async fn get(deployment: &Deployment, id: Uuid) -> Result<User, UserError> {
    let mut tx = deployment.begin().await?;
    let user = get_live(&mut tx, id).await?;
    tx.commit().await?;
    Ok(user)
}

/// Makes a user on the server with their email taken as proved, a plain
/// server write, never an event, since a device never creates one: for
/// a seed or a test. A sign-up goes through `accounts::sign_up`, which
/// makes the user unconfirmed with `create_in` until their code matches.
pub async fn create(deployment: &Deployment, email: &str) -> Result<User, UserError> {
    if deployment.role != Role::Server {
        return Err(UserError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let user = insert_in(&mut tx, email, true).await?;
    tx.commit().await?;
    Ok(user)
}

/// The user's row, unconfirmed, with their email as `checked_email`
/// keeps it.
pub(crate) async fn create_in(tx: &mut Transaction, email: &str) -> Result<User, UserError> {
    insert_in(tx, email, false).await
}

async fn insert_in(tx: &mut Transaction, email: &str, confirmed: bool) -> Result<User, UserError> {
    let email = checked_email(email)?;
    let now = now();
    let user = User {
        id: Uuid::now_v7(),
        email,
        inserted_at: now,
        updated_at: now,
        version: 1,
        last_synced_at: Some(now),
        deleted_at: None,
        deleted_by_id: None,
        confirmed_at: confirmed.then_some(now),
        avatar_url: None,
    };
    sql!(&mut *tx, users::insert(&user)).map_err(email_taken)?;
    Ok(user)
}

/// Marks the user's email as proved, now, and answers them as they are
/// then; a server write the devices pull like any other.
pub(crate) async fn confirm_in(tx: &mut Transaction, id: Uuid) -> Result<User, UserError> {
    sql!(&mut *tx, users::set_confirmed_at(id, now()))?;
    get_live(tx, id).await
}

/// Sets the user's picture, on the server, as an event in its log like
/// any change a person makes; the devices pull the row. A device never
/// writes it, so there is no merge, see `avatars`.
#[cfg_attr(not(feature = "object-storage"), allow(dead_code))]
pub(crate) async fn update_avatar_url_in(
    tx: &mut Transaction,
    id: Uuid,
    avatar_url: &str,
) -> Result<User, UserError> {
    let user = get_live(tx, id).await?;
    let now = now();
    let event = Event::update(
        TABLE,
        id,
        user.version,
        Fields::from_iter([("avatar_url".to_string(), json!(user.avatar_url))]),
        Fields::from_iter([("avatar_url".to_string(), json!(avatar_url))]),
        now,
    );
    let updated = User {
        avatar_url: Some(avatar_url.to_string()),
        updated_at: now,
        version: user.version + 1,
        last_synced_at: Some(now),
        ..user
    };
    sql!(&mut *tx, users::update(&updated))?;
    let answer = Answer::applied(
        event.id,
        updated.version,
        Conflicts::new(),
        table_row(&updated)?,
    );
    events::record(tx, id, &event, &answer).await?;
    Ok(updated)
}

/// Every user's picture, deleted users' too, since a deleted row keeps
/// its file until the purge removes the row.
#[cfg_attr(not(feature = "object-storage"), allow(dead_code))]
pub(crate) async fn list_avatar_urls_in(tx: &mut Transaction) -> Result<Vec<String>, UserError> {
    Ok(sql!(tx, users::list_avatar_urls())?)
}

/// Deletes the users who never proved their email and were made before
/// the time, with their credentials and tokens, for the purge; how many.
pub(crate) async fn delete_unconfirmed_before(
    tx: &mut Transaction,
    time: DateTime<Utc>,
) -> Result<u64, UserError> {
    Ok(sql!(&mut *tx, users::delete_unconfirmed_before(time))?)
}

/// The live user the token stands for, on the server; a token the
/// server does not know, or one that has expired, stands for nobody.
pub async fn get_by_token(deployment: &Deployment, token: &str) -> Result<User, UserError> {
    let (api_made_after, session_made_after) = tokens::made_after_to_be_valid(now());
    let mut tx = deployment.begin().await?;
    let user = sql!(
        &mut tx,
        users::get_by_token_hash(&tokens::hash(token), api_made_after, session_made_after)
    )?;
    tx.commit().await?;
    user.ok_or(UserError::UnknownToken)
}

/// The live user with the email as `checked_email` would keep it, on
/// the server, for a sign-in.
pub(crate) async fn get_by_email_in(
    tx: &mut Transaction,
    email: &str,
) -> Result<Option<User>, UserError> {
    Ok(sql!(
        &mut *tx,
        users::get_by_email(&normalized_email(email))
    )?)
}

/// The user signed in on this device, a device setting.
pub async fn signed_in(deployment: &Deployment) -> Result<User, UserError> {
    let (user, _token) = signed_in_with_token(deployment).await?;
    Ok(user)
}

/// The user signed in on this device, and the token the device sends
/// the server for them.
pub async fn signed_in_with_token(deployment: &Deployment) -> Result<(User, String), UserError> {
    let mut tx = deployment.begin().await?;
    let signed_in: Option<SignedIn> = device_settings::get(&mut tx, SIGNED_IN).await?;
    let signed_in = signed_in.ok_or(UserError::NotSignedIn)?;
    let user = get_live(&mut tx, signed_in.user_id).await?;
    tx.commit().await?;
    Ok((user, signed_in.token))
}

/// The user as they travel on the wire.
pub fn table_row(user: &User) -> Result<TableRow, UserError> {
    Ok(TableRow {
        table: TABLE.to_string(),
        row: serde_json::to_value(user)?,
    })
}

/// Writes the server's row over the device's: a user never has an event
/// pending, so it always applies.
pub(crate) async fn apply_server_row(tx: &mut Transaction, row: &Value) -> Result<bool, UserError> {
    let user: User = serde_json::from_value(row.clone())?;
    upsert_in(tx, &user).await?;
    Ok(true)
}

/// Writes the server's row, or inserts it.
pub(crate) async fn upsert_in(tx: &mut Transaction, user: &User) -> Result<(), UserError> {
    Ok(sql!(&mut *tx, users::upsert(user))?)
}

/// A page of the rows the server wrote after the cursor that the user
/// sees, deleted or not.
pub(crate) async fn list_synced_after(
    tx: &mut Transaction,
    user: &User,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<User>, UserError> {
    Ok(sql!(tx, users::list_synced_after(user.id, after, limit))?)
}

pub(crate) async fn get_in(tx: &mut Transaction, id: Uuid) -> Result<Option<User>, UserError> {
    Ok(sql!(tx, users::get(id))?)
}

pub(crate) async fn list_by_ids(
    tx: &mut Transaction,
    ids: &[Uuid],
) -> Result<Vec<User>, UserError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(sql!(tx, users::list_by_ids(ids))?)
}

async fn get_live(tx: &mut Transaction, id: Uuid) -> Result<User, UserError> {
    match get_in(tx, id).await? {
        Some(user) if user.deleted_at.is_none() => Ok(user),
        _ => Err(UserError::NotFound { id }),
    }
}

/// The email as it is kept: `normalized_email`, with an @ and no
/// spaces, and not too long.
fn checked_email(email: &str) -> Result<String, UserError> {
    let email = normalized_email(email);
    let (before, after) = email.split_once('@').ok_or(UserError::InvalidEmail)?;
    if before.is_empty()
        || after.is_empty()
        || email.chars().any(char::is_whitespace)
        || email.chars().count() > MAX_EMAIL_LENGTH
    {
        return Err(UserError::InvalidEmail);
    }
    Ok(email)
}

/// Trimmed and lowercased, so two spellings of one address are one
/// account, one inbox the code goes to, and one key the server limits.
fn normalized_email(email: &str) -> String {
    email.trim().to_lowercase()
}

fn email_taken(error: sqlx::Error) -> UserError {
    match &error {
        sqlx::Error::Database(database) if database.is_unique_violation() => UserError::EmailTaken,
        _ => UserError::Database(error),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::database::{Database, with_each_database};

    fn server(database: Database) -> Deployment {
        Deployment::new(database, Role::Server)
    }

    /// A device as the app opens it, with nobody signed in.
    pub(crate) async fn device() -> Deployment {
        let database = Database::open_sqlite_in_memory()
            .await
            .expect("an in-memory SQLite database");
        let deployment = Deployment::new(database, Role::Device);
        device_settings::upgrade(&deployment)
            .await
            .expect("the settings set up");
        deployment
    }

    /// A device after the user signed in on it with the token: their
    /// row is there and the setting names them, as `accounts` writes.
    pub(crate) async fn device_of(user: &User, token: &str) -> Deployment {
        let deployment = device().await;
        let mut tx = deployment.begin().await.expect("a transaction");
        upsert_in(&mut tx, user).await.expect("the user's row");
        let signed_in = SignedIn {
            user_id: user.id,
            token: token.to_string(),
        };
        device_settings::set(&mut tx, SIGNED_IN, &Some(signed_in))
            .await
            .expect("signed in");
        tx.commit().await.expect("committed");
        deployment
    }

    /// A user as the server would have made them, for a device test
    /// with no server.
    pub(crate) fn alice() -> User {
        let now = now();
        User {
            id: Uuid::now_v7(),
            email: "alice@example.com".to_string(),
            inserted_at: now,
            updated_at: now,
            version: 1,
            last_synced_at: Some(now),
            deleted_at: None,
            deleted_by_id: None,
            confirmed_at: Some(now),
            avatar_url: None,
        }
    }

    #[tokio::test]
    async fn users_create_on_the_server_is_version_one_and_found_by_id() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let user = create(&deployment, "  Alice@Example.com ")
                .await
                .expect("a user");
            assert_eq!(user.email, "alice@example.com", "trimmed and lowercased");
            assert_eq!(user.version, 1);
            assert_eq!(user.confirmed_at, Some(user.inserted_at));
            assert_eq!(user.last_synced_at, Some(user.updated_at));
            assert_eq!(get(&deployment, user.id).await.expect("the user"), user);

            let error = get(&deployment, Uuid::now_v7()).await.expect_err("missing");
            assert!(matches!(error, UserError::NotFound { .. }));
        })
        .await;
    }

    #[tokio::test]
    async fn users_create_refuses_an_invalid_or_taken_email() {
        with_each_database(|database| async move {
            let deployment = server(database);
            for email in [" ", "alice", "@example.com", "alice@", "al ice@example.com"] {
                let error = create(&deployment, email).await.expect_err("invalid");
                assert!(matches!(error, UserError::InvalidEmail), "{email}");
            }
            let long = format!("{}@example.com", "a".repeat(MAX_EMAIL_LENGTH));
            let error = create(&deployment, &long).await.expect_err("too long");
            assert!(matches!(error, UserError::InvalidEmail));

            create(&deployment, "alice@example.com")
                .await
                .expect("a user");
            let error = create(&deployment, "alice@example.com")
                .await
                .expect_err("taken");
            assert!(matches!(error, UserError::EmailTaken));
        })
        .await;
    }

    #[tokio::test]
    async fn users_create_is_the_servers_alone() {
        let deployment = device().await;
        let error = create(&deployment, "alice@example.com")
            .await
            .expect_err("a device");
        assert!(matches!(error, UserError::ServerOnly));
    }

    #[tokio::test]
    async fn users_signed_in_on_a_device_is_nobody_until_a_sign_in() {
        let deployment = device().await;
        let error = signed_in(&deployment).await.expect_err("nobody");
        assert!(matches!(error, UserError::NotSignedIn));

        let alice = alice();
        let deployment = device_of(&alice, "a token").await;
        let (user, token) = signed_in_with_token(&deployment)
            .await
            .expect("the signed-in user");
        assert_eq!(user, alice);
        assert_eq!(token, "a token");
    }
}
