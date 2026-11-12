//! The `accounts_subscriptions` table: Stripe's subscriptions, server only.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqliteConnection};
use uuid::Uuid;
use uuid::fmt::Hyphenated;

use crate::accounts::purchases::Store;
use crate::accounts::subscriptions::{Subscription, SubscriptionStatus};

const SELECT: &str = "SELECT accounts_subscriptions.id, user_id, product_id, price_id, store, store_id, status, \
     current_period_end, cancel_at_period_end, ended_at, accounts_subscriptions.inserted_at, \
     accounts_subscriptions.updated_at, accounts_products.name AS product_name \
     FROM accounts_subscriptions \
     JOIN accounts_products ON accounts_products.id = accounts_subscriptions.product_id";

#[derive(FromRow)]
struct SubscriptionRow {
    id: Hyphenated,
    user_id: Hyphenated,
    product_id: Hyphenated,
    price_id: Hyphenated,
    store: String,
    store_id: String,
    status: String,
    current_period_end: Option<DateTime<Utc>>,
    cancel_at_period_end: bool,
    ended_at: Option<DateTime<Utc>>,
    inserted_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    product_name: String,
}

impl TryFrom<SubscriptionRow> for (Subscription, String) {
    type Error = sqlx::Error;

    fn try_from(row: SubscriptionRow) -> Result<(Subscription, String), sqlx::Error> {
        let store = Store::from_name(&row.store)
            .ok_or_else(|| sqlx::Error::Decode(format!("no store {}", row.store).into()))?;
        let status = SubscriptionStatus::from_name(&row.status).ok_or_else(|| {
            sqlx::Error::Decode(format!("no subscription status {}", row.status).into())
        })?;
        let subscription = Subscription {
            id: row.id.into_uuid(),
            user_id: row.user_id.into_uuid(),
            product_id: row.product_id.into_uuid(),
            price_id: row.price_id.into_uuid(),
            store,
            store_id: row.store_id,
            status,
            current_period_end: row.current_period_end,
            cancel_at_period_end: row.cancel_at_period_end,
            ended_at: row.ended_at,
            inserted_at: row.inserted_at,
            updated_at: row.updated_at,
        };
        Ok((subscription, row.product_name))
    }
}

/// The subscription with its product's name.
pub async fn get(
    connection: &mut SqliteConnection,
    id: Uuid,
) -> Result<Option<(Subscription, String)>, sqlx::Error> {
    let row: Option<SubscriptionRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{SELECT} WHERE accounts_subscriptions.id = $1"
    )))
    .bind(id.hyphenated())
    .fetch_optional(connection)
    .await?;
    row.map(TryFrom::try_from).transpose()
}

/// The user's subscriptions with their products' names, newest first.
pub async fn list_by_user(
    connection: &mut SqliteConnection,
    user_id: Uuid,
) -> Result<Vec<(Subscription, String)>, sqlx::Error> {
    let rows: Vec<SubscriptionRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{SELECT} WHERE user_id = $1 \
         ORDER BY accounts_subscriptions.inserted_at DESC, accounts_subscriptions.id DESC"
    )))
    .bind(user_id.hyphenated())
    .fetch_all(connection)
    .await?;
    rows.into_iter().map(TryFrom::try_from).collect()
}

/// Inserts the subscription, or updates the one with its store's id,
/// keeping that one's id, user and `inserted_at`.
pub async fn upsert(
    connection: &mut SqliteConnection,
    subscription: &Subscription,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO accounts_subscriptions \
         (id, user_id, product_id, price_id, store, store_id, status, current_period_end, \
         cancel_at_period_end, ended_at, inserted_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) \
         ON CONFLICT (store, store_id) DO UPDATE SET product_id = excluded.product_id, \
         price_id = excluded.price_id, status = excluded.status, \
         current_period_end = excluded.current_period_end, \
         cancel_at_period_end = excluded.cancel_at_period_end, ended_at = excluded.ended_at, \
         updated_at = excluded.updated_at",
    )
    .bind(subscription.id.hyphenated())
    .bind(subscription.user_id.hyphenated())
    .bind(subscription.product_id.hyphenated())
    .bind(subscription.price_id.hyphenated())
    .bind(subscription.store.as_str())
    .bind(&subscription.store_id)
    .bind(subscription.status.as_str())
    .bind(subscription.current_period_end)
    .bind(subscription.cancel_at_period_end)
    .bind(subscription.ended_at)
    .bind(subscription.inserted_at)
    .bind(subscription.updated_at)
    .execute(connection)
    .await?;
    Ok(())
}
