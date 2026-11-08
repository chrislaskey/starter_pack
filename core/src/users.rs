//! Users: the app's view of each account, one row per person, written
//! by the server at registration and synced to devices like any row, so
//! a post can show who wrote it. Credentials never leave the server and
//! come with the login, later, in a table of their own with the same
//! ids. Every core function takes the `User` acting: the server resolves
//! it from the request's bearer token, `get_by_token`, and a device
//! keeps the one signed in as a device setting, `signed_in`.
//!
//! Until there is a login, every deployment acts as `dev_user`: the web
//! server answers a request with no token as it, and a device starts
//! signed in as it. The login replaces exactly those two places.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::{Uuid, uuid};

use crate::database::{Transaction, sql};
use crate::deployment::{Deployment, Role, now};
use crate::device_settings::{self, DeviceSettingsError, SIGNED_IN_USER};
use crate::sync::{Cursor, TableRow};

pub const TABLE: &str = "app_users";

/// The user every deployment acts as until there is a login.
pub const DEV_USER_ID: Uuid = uuid!("01900000-0000-7000-8000-0000000000de");

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
}

#[derive(Debug, Error)]
pub enum UserError {
    #[error("user {id} not found")]
    NotFound { id: Uuid },
    #[error("a user needs an email")]
    BlankEmail,
    #[error("that email already has an account")]
    EmailTaken,
    #[error("only the server creates users")]
    ServerOnly,
    #[error("The server did not accept this token")]
    UnknownToken,
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

/// The stand-in for a signed-in user until there is a login, the same
/// on every deployment, so the web and the apps act as one person.
pub fn dev_user() -> User {
    User {
        id: DEV_USER_ID,
        email: "dev@example.com".to_string(),
        inserted_at: DateTime::<Utc>::UNIX_EPOCH,
        updated_at: DateTime::<Utc>::UNIX_EPOCH,
        version: 0,
        last_synced_at: None,
        deleted_at: None,
        deleted_by_id: None,
    }
}

/// One user by their id, unless deleted.
pub async fn get(deployment: &Deployment, id: Uuid) -> Result<User, UserError> {
    let mut tx = deployment.begin().await?;
    let user = get_live(&mut tx, id).await?;
    tx.commit().await?;
    Ok(user)
}

/// Registers a user on the server: a plain server write, never an event,
/// since a device never creates one.
pub async fn create(deployment: &Deployment, email: &str) -> Result<User, UserError> {
    if deployment.role != Role::Server {
        return Err(UserError::ServerOnly);
    }
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
    };
    let mut tx = deployment.begin().await?;
    sql!(&mut tx, users::insert(&user)).map_err(email_taken)?;
    tx.commit().await?;
    Ok(user)
}

/// Writes the user's row when this deployment has none, as the role
/// writes a row: the server as a registration, a device as a copy the
/// next pull replaces. What makes sure the acting user's row is there
/// before their first post.
pub async fn create_if_missing(deployment: &Deployment, user: &User) -> Result<User, UserError> {
    let mut tx = deployment.begin().await?;
    if let Some(existing) = sql!(&mut tx, users::get(user.id))? {
        tx.commit().await?;
        return Ok(existing);
    }
    let now = now();
    let (version, last_synced_at) = match deployment.role {
        Role::Server => (1, Some(now)),
        Role::Device => (0, None),
    };
    let user = User {
        email: checked_email(&user.email)?,
        inserted_at: now,
        updated_at: now,
        version,
        last_synced_at,
        deleted_at: None,
        deleted_by_id: None,
        ..user.clone()
    };
    sql!(&mut tx, users::insert(&user)).map_err(email_taken)?;
    tx.commit().await?;
    Ok(user)
}

/// The user a bearer token stands for, on the server.
pub async fn get_by_token(deployment: &Deployment, token: &str) -> Result<User, UserError> {
    let mut tx = deployment.begin().await?;
    let user = sql!(&mut tx, users::get_by_token(token))?;
    tx.commit().await?;
    user.ok_or(UserError::UnknownToken)
}

/// The user signed in on this device, a device setting.
pub async fn signed_in(deployment: &Deployment) -> Result<User, UserError> {
    let mut tx = deployment.begin().await?;
    let user = device_settings::get(&mut tx, SIGNED_IN_USER).await?;
    tx.commit().await?;
    Ok(user)
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
    sql!(&mut *tx, users::upsert(&user))?;
    Ok(true)
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

/// The email as it is kept: trimmed, and not blank.
fn checked_email(email: &str) -> Result<String, UserError> {
    let email = email.trim();
    if email.is_empty() {
        return Err(UserError::BlankEmail);
    }
    Ok(email.to_string())
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

    #[tokio::test]
    async fn users_create_on_the_server_is_version_one_and_found_by_id() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let user = create(&deployment, "  alice@example.com ")
                .await
                .expect("a user");
            assert_eq!(user.email, "alice@example.com");
            assert_eq!(user.version, 1);
            assert_eq!(user.last_synced_at, Some(user.updated_at));
            assert_eq!(get(&deployment, user.id).await.expect("the user"), user);

            let error = get(&deployment, Uuid::now_v7()).await.expect_err("missing");
            assert!(matches!(error, UserError::NotFound { .. }));
        })
        .await;
    }

    #[tokio::test]
    async fn users_create_refuses_a_blank_or_taken_email() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let error = create(&deployment, " ").await.expect_err("blank");
            assert!(matches!(error, UserError::BlankEmail));

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
    async fn users_create_if_missing_writes_the_row_once_as_the_role_does() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let user = create_if_missing(&deployment, &dev_user())
                .await
                .expect("the dev user");
            assert_eq!(user.id, DEV_USER_ID);
            assert_eq!(user.version, 1);
            assert!(user.last_synced_at.is_some());
            let again = create_if_missing(&deployment, &dev_user())
                .await
                .expect("the dev user again");
            assert_eq!(again, user);
        })
        .await;

        let deployment = device().await;
        let user = create_if_missing(&deployment, &dev_user())
            .await
            .expect("the dev user");
        assert_eq!(user.version, 0);
        assert_eq!(user.last_synced_at, None);
    }

    #[tokio::test]
    async fn users_signed_in_on_a_device_is_the_dev_user_until_there_is_a_login() {
        let deployment = device().await;
        let user = signed_in(&deployment).await.expect("the signed-in user");
        assert_eq!(user, dev_user());
    }
}
