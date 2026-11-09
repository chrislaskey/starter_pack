//! The `app_users` table. Ids are UUIDv7 text, so the file reads well in
//! the sqlite3 shell; the row type here turns them into `User`'s `Uuid`.

use chrono::{DateTime, Utc};
use sqlx::types::Json;
use sqlx::{FromRow, SqliteConnection};
use uuid::Uuid;
use uuid::fmt::Hyphenated;

use crate::sync::Cursor;
use crate::users::User;

#[derive(FromRow)]
struct UserRow {
    id: Hyphenated,
    email: String,
    inserted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    version: i64,
    last_synced_at: Option<DateTime<Utc>>,
    deleted_at: Option<DateTime<Utc>>,
    deleted_by_id: Option<Hyphenated>,
    confirmed_at: Option<DateTime<Utc>>,
    avatar_url: Option<String>,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> User {
        User {
            id: row.id.into_uuid(),
            email: row.email,
            inserted_at: row.inserted_at,
            updated_at: row.updated_at,
            version: row.version,
            last_synced_at: row.last_synced_at,
            deleted_at: row.deleted_at,
            deleted_by_id: row.deleted_by_id.map(Hyphenated::into_uuid),
            confirmed_at: row.confirmed_at,
            avatar_url: row.avatar_url,
        }
    }
}

macro_rules! select {
    ($($rest:expr),+ $(,)?) => {
        concat!(
            "SELECT id, email, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id, confirmed_at, avatar_url FROM app_users ",
            $($rest),+
        )
    };
}

/// The row by its id, deleted or not.
pub async fn get(connection: &mut SqliteConnection, id: Uuid) -> Result<Option<User>, sqlx::Error> {
    let row: Option<UserRow> = sqlx::query_as(select!("WHERE id = $1"))
        .bind(id.hyphenated())
        .fetch_optional(connection)
        .await?;
    Ok(row.map(User::from))
}

/// The live user the token with the hash stands for: an app's token
/// made after `api_made_after`, or the browser's made after
/// `session_made_after`, which is when each would have expired.
pub async fn get_by_token_hash(
    connection: &mut SqliteConnection,
    token_hash: &str,
    api_made_after: DateTime<Utc>,
    session_made_after: DateTime<Utc>,
) -> Result<Option<User>, sqlx::Error> {
    let row: Option<UserRow> = sqlx::query_as(
        "SELECT u.id, u.email, u.inserted_at, u.updated_at, u.version, u.last_synced_at, u.deleted_at, u.deleted_by_id, u.confirmed_at, u.avatar_url
         FROM app_users u JOIN users_tokens t ON t.user_id = u.id
         WHERE t.token_hash = $1 AND u.deleted_at IS NULL
           AND ((t.context = 'api' AND t.inserted_at > $2) OR (t.context = 'session' AND t.inserted_at > $3))",
    )
    .bind(token_hash)
    .bind(api_made_after)
    .bind(session_made_after)
    .fetch_optional(connection)
    .await?;
    Ok(row.map(User::from))
}

/// Marks the user's email as proved, as a write of the server's that
/// the devices pull like any other.
pub async fn set_confirmed_at(
    connection: &mut SqliteConnection,
    id: Uuid,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE app_users SET confirmed_at = $2, updated_at = $2, last_synced_at = $2, version = version + 1
         WHERE id = $1",
    )
    .bind(id.hyphenated())
    .bind(now)
    .execute(connection)
    .await?;
    Ok(())
}

/// Writes the fields a user's event may change, with the sync columns.
#[cfg_attr(not(feature = "object-storage"), allow(dead_code))]
pub async fn update(connection: &mut SqliteConnection, user: &User) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE app_users SET avatar_url = $2, updated_at = $3, version = $4, last_synced_at = $5
         WHERE id = $1",
    )
    .bind(user.id.hyphenated())
    .bind(&user.avatar_url)
    .bind(user.updated_at)
    .bind(user.version)
    .bind(user.last_synced_at)
    .execute(connection)
    .await?;
    Ok(())
}

/// Every user's picture, deleted users' too, for the sweep.
#[cfg_attr(not(feature = "object-storage"), allow(dead_code))]
pub async fn list_avatar_urls(
    connection: &mut SqliteConnection,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT avatar_url FROM app_users WHERE avatar_url IS NOT NULL")
        .fetch_all(connection)
        .await
}

/// Deletes the users who never proved their email and were made before
/// the time, with their credentials and tokens; how many.
pub async fn delete_unconfirmed_before(
    connection: &mut SqliteConnection,
    time: DateTime<Utc>,
) -> Result<u64, sqlx::Error> {
    let result =
        sqlx::query("DELETE FROM app_users WHERE confirmed_at IS NULL AND inserted_at < $1")
            .bind(time)
            .execute(connection)
            .await?;
    Ok(result.rows_affected())
}

/// The live user with the email.
pub async fn get_by_email(
    connection: &mut SqliteConnection,
    email: &str,
) -> Result<Option<User>, sqlx::Error> {
    let row: Option<UserRow> = sqlx::query_as(select!("WHERE email = $1 AND deleted_at IS NULL"))
        .bind(email)
        .fetch_optional(connection)
        .await?;
    Ok(row.map(User::from))
}

/// The users the user, `$1`, sees: themselves, and the live members of
/// the communities they are a live member of.
macro_rules! visible_to {
    () => {
        "(id = $1 OR id IN (
            SELECT uc.user_id FROM app_users_communities uc
            WHERE uc.deleted_at IS NULL AND uc.community_id IN (SELECT community_id FROM app_users_communities WHERE user_id = $1 AND deleted_at IS NULL)))"
    };
}

/// The rows with the ids, deleted or not.
pub async fn list_by_ids(
    connection: &mut SqliteConnection,
    ids: &[Uuid],
) -> Result<Vec<User>, sqlx::Error> {
    let rows: Vec<UserRow> =
        sqlx::query_as(select!("WHERE id IN (SELECT value FROM json_each($1))"))
            .bind(Json(
                ids.iter()
                    .map(|id| id.hyphenated().to_string())
                    .collect::<Vec<String>>(),
            ))
            .fetch_all(connection)
            .await?;
    Ok(rows.into_iter().map(User::from).collect())
}

/// The rows the server wrote after the cursor that the user sees, in
/// cursor order, deleted or not: a page of a pull.
pub async fn list_synced_after(
    connection: &mut SqliteConnection,
    user_id: Uuid,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<User>, sqlx::Error> {
    let rows: Vec<UserRow> = sqlx::query_as(select!(
        "WHERE last_synced_at IS NOT NULL AND (last_synced_at, id) > ($2, $3) AND ",
        visible_to!(),
        " ORDER BY last_synced_at, id LIMIT $4"
    ))
    .bind(user_id.hyphenated())
    .bind(after.last_synced_at)
    .bind(after.id.hyphenated())
    .bind(i64::try_from(limit).unwrap_or(i64::MAX))
    .fetch_all(connection)
    .await?;
    Ok(rows.into_iter().map(User::from).collect())
}

pub async fn insert(connection: &mut SqliteConnection, user: &User) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_users (id, email, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id, confirmed_at, avatar_url)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(user.id.hyphenated())
    .bind(&user.email)
    .bind(user.inserted_at)
    .bind(user.updated_at)
    .bind(user.version)
    .bind(user.last_synced_at)
    .bind(user.deleted_at)
    .bind(user.deleted_by_id.map(Uuid::hyphenated))
    .bind(user.confirmed_at)
    .bind(&user.avatar_url)
    .execute(connection)
    .await?;
    Ok(())
}

/// Writes the server's row over the device's, or inserts it.
pub async fn upsert(connection: &mut SqliteConnection, user: &User) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_users (id, email, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id, confirmed_at, avatar_url)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         ON CONFLICT (id) DO UPDATE SET
           email = excluded.email, inserted_at = excluded.inserted_at, updated_at = excluded.updated_at,
           version = excluded.version, last_synced_at = excluded.last_synced_at,
           deleted_at = excluded.deleted_at, deleted_by_id = excluded.deleted_by_id,
           confirmed_at = excluded.confirmed_at, avatar_url = excluded.avatar_url",
    )
    .bind(user.id.hyphenated())
    .bind(&user.email)
    .bind(user.inserted_at)
    .bind(user.updated_at)
    .bind(user.version)
    .bind(user.last_synced_at)
    .bind(user.deleted_at)
    .bind(user.deleted_by_id.map(Uuid::hyphenated))
    .bind(user.confirmed_at)
    .bind(&user.avatar_url)
    .execute(connection)
    .await?;
    Ok(())
}
