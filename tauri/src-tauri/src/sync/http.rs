//! The three sync endpoints of `App.Web.SyncController`, on the realm's
//! host with the API token as a bearer token.

use std::time::Duration;

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::store::{Cursor, WireRow};
use crate::config::Config;

const PAGE_SIZE: usize = 200;
const TIMEOUT: Duration = Duration::from_secs(60);

pub struct SyncServer {
    client: reqwest::Client,
    config: Config,
    host: String,
    token: String,
}

/// One page of a pull or a backfill.
#[derive(Debug, Deserialize)]
pub struct Page {
    pub rows: Vec<WireRow>,
    /// The token of the next page, or nothing on the last one.
    pub next: Option<Value>,
    /// What to store as the cursor once `next` is nothing.
    pub cursor: Option<Cursor>,
}

#[derive(Debug, Deserialize)]
pub struct PushResponse {
    pub accepted: Vec<Accepted>,
    pub rejected: Vec<Rejected>,
}

#[derive(Debug, Deserialize)]
pub struct Accepted {
    pub table: String,
    pub id: String,
    pub last_synced_at: String,
}

#[derive(Debug, Deserialize)]
pub struct Rejected {
    pub table: String,
    pub id: String,
    pub reason: String,
    /// The server's version of the row, to apply, or nothing when the
    /// server has none.
    pub server: Option<WireRow>,
}

impl SyncServer {
    pub fn new(config: &Config, realm_host: &str, token: &str) -> Result<Self, String> {
        let mut builder = reqwest::Client::builder().timeout(TIMEOUT);
        if let Some(address) = config.dev_address(realm_host) {
            builder = builder.resolve(realm_host, address);
        }
        let client = builder.build().map_err(|error| error.to_string())?;

        Ok(Self {
            client,
            config: config.clone(),
            host: realm_host.to_string(),
            token: token.to_string(),
        })
    }

    pub async fn pull(
        &self,
        cursor: Option<&Cursor>,
        after: Option<&Value>,
    ) -> Result<Page, String> {
        self.post(
            "/sync/pull",
            &json!({"cursor": cursor, "after": after, "limit": PAGE_SIZE}),
        )
        .await
    }

    pub async fn push(&self, rows: Vec<WireRow>) -> Result<PushResponse, String> {
        self.post("/sync/push", &json!({"rows": rows})).await
    }

    pub async fn backfill(
        &self,
        community_id: &str,
        after: Option<&Value>,
    ) -> Result<Page, String> {
        self.post(
            &format!("/sync/backfill/{community_id}"),
            &json!({"after": after, "limit": PAGE_SIZE}),
        )
        .await
    }

    async fn post<T: DeserializeOwned>(&self, path: &str, body: &Value) -> Result<T, String> {
        let url = self.config.sync_url(&self.host, path)?;
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.token)
            .json(body)
            .send()
            .await
            .map_err(|error| format!("Could not reach {}: {error}", self.host))?;

        let status = response.status();
        if !status.is_success() {
            return Err(format!("{} refused {path} with {status}", self.host));
        }
        response
            .json()
            .await
            .map_err(|error| format!("{path} replied with something unexpected: {error}"))
    }
}
