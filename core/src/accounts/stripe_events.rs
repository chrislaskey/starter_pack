//! The events Stripe sends to the webhook, one row of
//! `accounts_stripe_events` per event handled. Server only. Stripe may
//! send an event more than once, so a repeat is recognized by its id and
//! skipped; the row is written in the same transaction as what the event
//! changes, so an event that fails is not recorded and Stripe sends it
//! again. See `docs/design/purchasing.md`.

use chrono::{DateTime, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use stripe_webhook::{Webhook, WebhookError};
use thiserror::Error;

use crate::database::sql;
use crate::deployment::{Deployment, Role, now};

pub const TABLE: &str = "accounts_stripe_events";

/// An event handled, by Stripe's id, `evt_...`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StripeEvent {
    pub id: String,
    /// Stripe's name for it, `checkout.session.completed`.
    pub event_type: String,
    pub handled_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct EventIdAndType {
    id: String,
    #[serde(rename = "type")]
    event_type: String,
}

/// What `create` did with a delivery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StripeEventCreated {
    /// The first delivery of the event: handled now.
    New(StripeEvent),
    /// A delivery of an event handled before: skipped.
    Repeat(StripeEvent),
}

#[derive(Debug, Error)]
pub enum StripeEventError {
    #[error("only the server takes Stripe's events")]
    ServerOnly,
    #[error("the Stripe-Signature does not match the body: {0}")]
    BadSignature(String),
    #[error("the event could not be read: {0}")]
    Unreadable(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Checks a delivery's `Stripe-Signature` header against its body, as
/// sent, with the endpoint's signing secret, `whsec_...`, then records
/// the event. A signature more than five minutes old is refused.
pub async fn create(
    deployment: &Deployment,
    signing_secret: &SecretString,
    body: &str,
    signature: &str,
) -> Result<StripeEventCreated, StripeEventError> {
    if deployment.role != Role::Server {
        return Err(StripeEventError::ServerOnly);
    }
    let verified = Webhook::construct_event_with_timestamp(
        body,
        signature,
        signing_secret.expose_secret(),
        now().timestamp(),
    );
    let EventIdAndType { id, event_type } = match verified {
        Ok(event) => EventIdAndType {
            id: event.id.as_str().to_string(),
            event_type: event.type_.as_str().to_string(),
        },
        // The crate checks the signature before it parses, so a body it
        // cannot type was signed; only the id and type are needed, as the
        // object is fetched from Stripe.
        Err(WebhookError::BadParse(_)) => serde_json::from_str(body)
            .map_err(|error| StripeEventError::Unreadable(error.to_string()))?,
        Err(other) => return Err(StripeEventError::BadSignature(other.to_string())),
    };
    let id = id.as_str();
    let event_type = event_type.as_str();

    let mut tx = deployment.begin().await?;
    let inserted = sql!(
        &mut tx,
        accounts_stripe_events::insert_if_missing(id, event_type, now())
    )?;
    let recorded = sql!(&mut tx, accounts_stripe_events::get(id))?;
    tx.commit().await?;
    let recorded = recorded.ok_or(sqlx::Error::RowNotFound)?;
    if inserted == 0 {
        tracing::info!("stripe event {id} {event_type}: a repeat, skipped");
        return Ok(StripeEventCreated::Repeat(recorded));
    }
    tracing::info!("stripe event {id} {event_type}: recorded");
    Ok(StripeEventCreated::New(recorded))
}

/// The event handled with the id, if it was.
pub async fn get(
    deployment: &Deployment,
    id: &str,
) -> Result<Option<StripeEvent>, StripeEventError> {
    if deployment.role != Role::Server {
        return Err(StripeEventError::ServerOnly);
    }
    let mut tx = deployment.begin().await?;
    let event = sql!(&mut tx, accounts_stripe_events::get(id))?;
    tx.commit().await?;
    Ok(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::with_each_database;

    const SIGNING_SECRET: &str = "whsec_test_secret";

    /// An event as Stripe sends it, with an object no Stripe type has: a
    /// known event type the crate cannot type, an unknown one it keeps.
    fn event_body(id: &str, event_type: &str) -> String {
        serde_json::json!({
            "id": id,
            "object": "event",
            "api_version": "2026-09-30.endive",
            "created": now().timestamp(),
            "data": { "object": { "id": "obj_test", "object": "test_object" } },
            "livemode": false,
            "pending_webhooks": 1,
            "request": { "id": null, "idempotency_key": null },
            "type": event_type,
        })
        .to_string()
    }

    /// The `Stripe-Signature` Stripe would send with the body, signed so
    /// many seconds ago.
    fn signature_for(body: &str, secret: &str, seconds_ago: i64) -> String {
        Webhook::generate_test_header(body, secret, Some(now().timestamp() - seconds_ago))
    }

    fn secret() -> SecretString {
        SecretString::from(SIGNING_SECRET)
    }

    #[tokio::test]
    async fn stripe_events_create_records_an_event_once() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let body = event_body("evt_1", "customer.created");
            let signature = signature_for(&body, SIGNING_SECRET, 0);

            let created = create(&deployment, &secret(), &body, &signature)
                .await
                .expect("recorded");
            let StripeEventCreated::New(event) = created else {
                panic!("the first delivery is new");
            };
            assert_eq!(event.id, "evt_1");
            assert_eq!(event.event_type, "customer.created");
            let again = create(&deployment, &secret(), &body, &signature)
                .await
                .expect("a repeat");
            assert_eq!(again, StripeEventCreated::Repeat(event.clone()));
            let got = get(&deployment, "evt_1").await.expect("read");
            assert_eq!(got, Some(event));

            let body = event_body("evt_2", "some.future_event");
            let signature = signature_for(&body, SIGNING_SECRET, 0);
            let created = create(&deployment, &secret(), &body, &signature)
                .await
                .expect("recorded");
            let StripeEventCreated::New(event) = created else {
                panic!("the first delivery is new");
            };
            assert_eq!(event.event_type, "some.future_event");
        })
        .await;
    }

    #[tokio::test]
    async fn stripe_events_create_refuses_a_bad_signature() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let body = event_body("evt_1", "customer.created");
            let cases = [
                signature_for(&body, "whsec_another_secret", 0),
                signature_for(&event_body("evt_2", "customer.created"), SIGNING_SECRET, 0),
                signature_for(&body, SIGNING_SECRET, 10 * 60),
                "not a signature".to_string(),
            ];
            for signature in cases {
                let error = create(&deployment, &secret(), &body, &signature)
                    .await
                    .expect_err("refused");
                assert!(matches!(error, StripeEventError::BadSignature(_)));
            }
            assert_eq!(get(&deployment, "evt_1").await.expect("read"), None);
        })
        .await;
    }

    #[tokio::test]
    async fn stripe_events_create_refuses_a_body_that_is_no_event() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Server);
            let body = r#"{"hello": "world"}"#;
            let signature = signature_for(body, SIGNING_SECRET, 0);
            let error = create(&deployment, &secret(), body, &signature)
                .await
                .expect_err("unreadable");
            assert!(matches!(error, StripeEventError::Unreadable(_)));
        })
        .await;
    }

    #[tokio::test]
    async fn stripe_events_are_the_servers_alone() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Device);
            let body = event_body("evt_1", "customer.created");
            let signature = signature_for(&body, SIGNING_SECRET, 0);
            let error = create(&deployment, &secret(), &body, &signature)
                .await
                .expect_err("server only");
            assert!(matches!(error, StripeEventError::ServerOnly));
        })
        .await;
    }
}
