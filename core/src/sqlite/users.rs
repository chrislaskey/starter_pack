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
        }
    }
}

macro_rules! select {
    ($($rest:expr),+ $(,)?) => {
        concat!(
            "SELECT id, email, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id FROM app_users ",
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

/// The live user the token stands for.
pub async fn get_by_token(
    connection: &mut SqliteConnection,
    token: &str,
) -> Result<Option<User>, sqlx::Error> {
    let row: Option<UserRow> = sqlx::query_as(
        "SELECT u.id, u.email, u.inserted_at, u.updated_at, u.version, u.last_synced_at, u.deleted_at, u.deleted_by_id
         FROM app_users u JOIN users_tokens t ON t.user_id = u.id
         WHERE t.token = $1 AND u.deleted_at IS NULL",
    )
    .bind(token)
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
        "INSERT INTO app_users (id, email, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(user.id.hyphenated())
    .bind(&user.email)
    .bind(user.inserted_at)
    .bind(user.updated_at)
    .bind(user.version)
    .bind(user.last_synced_at)
    .bind(user.deleted_at)
    .bind(user.deleted_by_id.map(Uuid::hyphenated))
    .execute(connection)
    .await?;
    Ok(())
}

/// Writes the server's row over the device's, or inserts it.
pub async fn upsert(connection: &mut SqliteConnection, user: &User) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO app_users (id, email, inserted_at, updated_at, version, last_synced_at, deleted_at, deleted_by_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (id) DO UPDATE SET
           email = excluded.email, inserted_at = excluded.inserted_at, updated_at = excluded.updated_at,
           version = excluded.version, last_synced_at = excluded.last_synced_at,
           deleted_at = excluded.deleted_at, deleted_by_id = excluded.deleted_by_id",
    )
    .bind(user.id.hyphenated())
    .bind(&user.email)
    .bind(user.inserted_at)
    .bind(user.updated_at)
    .bind(user.version)
    .bind(user.last_synced_at)
    .bind(user.deleted_at)
    .bind(user.deleted_by_id.map(Uuid::hyphenated))
    .execute(connection)
    .await?;
    Ok(())
}
