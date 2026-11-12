//! Plans paid every month or year: Stripe's subscriptions, one row of
//! `accounts_subscriptions` each, copied from Stripe when a plan's
//! checkout is paid and on every `customer.subscription.*` event, so a
//! change made anywhere, on Stripe's Link or by Stripe retrying a card,
//! lands here. Server only. Only an active subscription, or one in its
//! free days, gives its plan, see `plans`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::accounts::prices::{self, Price};
use crate::accounts::purchases::Store;
use crate::accounts::stripe::StripeSubscription;
use crate::database::{Transaction, sql};
use crate::deployment::{Deployment, Role};
use crate::users::User;

pub const TABLE: &str = "accounts_subscriptions";

/// Stripe's statuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Active,
    /// In its free days.
    Trialing,
    /// A renewal's payment failed, and Stripe is retrying it.
    PastDue,
    /// Stripe stopped retrying.
    Unpaid,
    /// The first payment is not made yet.
    Incomplete,
    IncompleteExpired,
    Paused,
    Canceled,
}

impl SubscriptionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SubscriptionStatus::Active => "active",
            SubscriptionStatus::Trialing => "trialing",
            SubscriptionStatus::PastDue => "past_due",
            SubscriptionStatus::Unpaid => "unpaid",
            SubscriptionStatus::Incomplete => "incomplete",
            SubscriptionStatus::IncompleteExpired => "incomplete_expired",
            SubscriptionStatus::Paused => "paused",
            SubscriptionStatus::Canceled => "canceled",
        }
    }

    pub fn from_name(name: &str) -> Option<SubscriptionStatus> {
        match name {
            "active" => Some(SubscriptionStatus::Active),
            "trialing" => Some(SubscriptionStatus::Trialing),
            "past_due" => Some(SubscriptionStatus::PastDue),
            "unpaid" => Some(SubscriptionStatus::Unpaid),
            "incomplete" => Some(SubscriptionStatus::Incomplete),
            "incomplete_expired" => Some(SubscriptionStatus::IncompleteExpired),
            "paused" => Some(SubscriptionStatus::Paused),
            "canceled" => Some(SubscriptionStatus::Canceled),
            _ => None,
        }
    }

    /// Whether it gives its plan: paid, or in its free days. A plan not
    /// paid is no plan.
    pub fn gives_plan(self) -> bool {
        matches!(
            self,
            SubscriptionStatus::Active | SubscriptionStatus::Trialing
        )
    }

    /// Whether a payment failed and the person can fix it with another card.
    pub fn is_payment_failed(self) -> bool {
        matches!(
            self,
            SubscriptionStatus::PastDue | SubscriptionStatus::Unpaid
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscription {
    pub id: Uuid,
    pub user_id: Uuid,
    pub product_id: Uuid,
    pub price_id: Uuid,
    pub store: Store,
    /// The store's id, `sub_...` for Stripe.
    pub store_id: String,
    pub status: SubscriptionStatus,
    /// When the period paid for ends: the next renewal, or the end.
    pub current_period_end: Option<DateTime<Utc>>,
    pub cancel_at_period_end: bool,
    pub ended_at: Option<DateTime<Utc>>,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A subscription as its pages show it, with its plan and price.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionShown {
    #[serde(flatten)]
    pub subscription: Subscription,
    pub product_name: String,
    pub price: Price,
    pub gives_plan: bool,
}

#[derive(Debug, Error)]
pub enum SubscriptionError {
    #[error("only the server has subscriptions")]
    ServerOnly,
    #[error("No subscription {id} of yours")]
    NotFound { id: Uuid },
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// The user's subscriptions, the newest first, ended ones too.
pub async fn list(
    deployment: &Deployment,
    user: &User,
) -> Result<Vec<SubscriptionShown>, SubscriptionError> {
    if deployment.role != Role::Server {
        return Err(SubscriptionError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let rows = sql!(&mut tx, accounts_subscriptions::list_by_user(user.id))?;
    let mut shown = Vec::with_capacity(rows.len());
    for (subscription, product_name) in rows {
        shown.push(shown_in(&mut tx, subscription, product_name).await?);
    }
    tx.commit().await?;
    Ok(shown)
}

/// One of the user's subscriptions.
pub async fn get(
    deployment: &Deployment,
    user: &User,
    id: Uuid,
) -> Result<SubscriptionShown, SubscriptionError> {
    if deployment.role != Role::Server {
        return Err(SubscriptionError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let row = sql!(&mut tx, accounts_subscriptions::get(id))?
        .filter(|(subscription, _)| subscription.user_id == user.id)
        .ok_or(SubscriptionError::NotFound { id })?;
    let shown = shown_in(&mut tx, row.0, row.1).await?;
    tx.commit().await?;
    Ok(shown)
}

/// The user's subscriptions, the newest first, for `plans`.
pub(crate) async fn list_in(
    tx: &mut Transaction,
    user_id: Uuid,
) -> Result<Vec<(Subscription, String)>, sqlx::Error> {
    sql!(&mut *tx, accounts_subscriptions::list_by_user(user_id))
}

/// The subscription as Stripe has it now: made, or updated in place by
/// its Stripe id. One whose customer is no account here, or whose price
/// is not in the catalog, is left out.
pub(crate) async fn upsert_from_stripe_in(
    tx: &mut Transaction,
    stripe_subscription: &StripeSubscription,
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    let user_id = sql!(
        &mut *tx,
        accounts_users::get_id_by_stripe_customer_id(&stripe_subscription.customer_id)
    )?;
    let price = sql!(
        &mut *tx,
        accounts_prices::get_by_stripe_id(&stripe_subscription.price_id)
    )?;
    let (Some(user_id), Some(price)) = (user_id, price) else {
        tracing::warn!(
            "stripe subscription {} is for no account or price here, left out",
            stripe_subscription.id
        );
        return Ok(());
    };
    let Some(status) = SubscriptionStatus::from_name(&stripe_subscription.status) else {
        tracing::warn!(
            "stripe subscription {} has a status unknown here, {}, left out",
            stripe_subscription.id,
            stripe_subscription.status
        );
        return Ok(());
    };
    let subscription = Subscription {
        id: Uuid::now_v7(),
        user_id,
        product_id: price.product_id,
        price_id: price.id,
        store: Store::Stripe,
        store_id: stripe_subscription.id.clone(),
        status,
        current_period_end: stripe_subscription
            .current_period_end
            .and_then(|seconds| DateTime::from_timestamp(seconds, 0)),
        cancel_at_period_end: stripe_subscription.cancel_at_period_end,
        ended_at: stripe_subscription
            .ended_at
            .and_then(|seconds| DateTime::from_timestamp(seconds, 0)),
        inserted_at: now,
        updated_at: now,
    };
    sql!(&mut *tx, accounts_subscriptions::upsert(&subscription))
}

async fn shown_in(
    tx: &mut Transaction,
    subscription: Subscription,
    product_name: String,
) -> Result<SubscriptionShown, sqlx::Error> {
    let price = prices::get_in(tx, subscription.price_id)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
    let gives_plan = subscription.status.gives_plan();
    Ok(SubscriptionShown {
        subscription,
        product_name,
        price,
        gives_plan,
    })
}
