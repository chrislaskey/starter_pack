//! One-time purchases, one row of `accounts_purchases` each, written when
//! a checkout is paid, see `checkouts`. Server only. A purchase records
//! the store that sold it, Stripe now, the phone stores later, and the
//! store's id for it, which is how a repeat of the same payment is
//! recognized. A refunded purchase gives nothing.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::database::{Transaction, sql};
use crate::deployment::{Deployment, Role};
use crate::users::User;

pub const TABLE: &str = "accounts_purchases";

/// Who sold it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Store {
    Stripe,
    AppStore,
    PlayStore,
}

impl Store {
    pub fn as_str(self) -> &'static str {
        match self {
            Store::Stripe => "stripe",
            Store::AppStore => "app_store",
            Store::PlayStore => "play_store",
        }
    }

    pub fn from_name(name: &str) -> Option<Store> {
        match name {
            "stripe" => Some(Store::Stripe),
            "app_store" => Some(Store::AppStore),
            "play_store" => Some(Store::PlayStore),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Purchase {
    pub id: Uuid,
    pub user_id: Uuid,
    pub product_id: Uuid,
    pub price_id: Uuid,
    /// The checkout that paid it; none for a phone store's.
    pub checkout_id: Option<Uuid>,
    pub store: Store,
    /// The store's id for the payment, `pi_...` for Stripe.
    pub store_id: String,
    /// What was paid, in the currency's smallest unit, tax included.
    pub amount: i64,
    pub currency: String,
    pub refunded_at: Option<DateTime<Utc>>,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A purchase as its pages show it, with what was bought.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurchaseShown {
    #[serde(flatten)]
    pub purchase: Purchase,
    pub product_name: String,
}

#[derive(Debug, Error)]
pub enum PurchaseError {
    #[error("only the server has purchases")]
    ServerOnly,
    #[error("No purchase {id} of yours")]
    NotFound { id: Uuid },
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// The user's purchases, newest first, refunded ones too.
pub async fn list(
    deployment: &Deployment,
    user: &User,
) -> Result<Vec<PurchaseShown>, PurchaseError> {
    if deployment.role != Role::Server {
        return Err(PurchaseError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let purchases = sql!(&mut tx, accounts_purchases::list_by_user(user.id))?;
    tx.commit().await?;
    Ok(purchases)
}

/// One of the user's purchases.
pub async fn get(
    deployment: &Deployment,
    user: &User,
    id: Uuid,
) -> Result<PurchaseShown, PurchaseError> {
    if deployment.role != Role::Server {
        return Err(PurchaseError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let purchase = sql!(&mut tx, accounts_purchases::get(id))?;
    tx.commit().await?;
    purchase
        .filter(|shown| shown.purchase.user_id == user.id)
        .ok_or(PurchaseError::NotFound { id })
}

/// Records the purchase, unless the store's payment is recorded already.
pub(crate) async fn insert_if_missing_in(
    tx: &mut Transaction,
    purchase: &Purchase,
) -> Result<(), sqlx::Error> {
    sql!(&mut *tx, accounts_purchases::insert_if_missing(purchase))
}

/// Whether the user has the product, bought and not refunded.
pub(crate) async fn owns_in(
    tx: &mut Transaction,
    user_id: Uuid,
    product_id: Uuid,
) -> Result<bool, sqlx::Error> {
    sql!(&mut *tx, accounts_purchases::owns(user_id, product_id))
}

/// The store's payment refunded in full: the purchase gives nothing now.
pub(crate) async fn refund_in(
    tx: &mut Transaction,
    store: Store,
    store_id: &str,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sql!(&mut *tx, accounts_purchases::refund(store, store_id, now))
}
