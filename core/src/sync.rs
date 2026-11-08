//! Sync: how a device's database and the web server's converge, see
//! `docs/design/offline-sync.md`. The server answers a push of events and
//! a pull of rows since a cursor, `push` and `pull`, which the web server
//! serves under `/api/sync`. A device does one sync with `run`: it pushes
//! its pending events through a `Transport`, applies the answers, then
//! pulls pages from its cursor. The transport is HTTP in the apps, behind
//! the `sync-http` feature, and the server's own functions in this
//! process in tests. A device that chose to work offline, `set_offline`, a
//! device setting, makes no request at all until it chooses otherwise.

use std::collections::BTreeMap;
use std::future::Future;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::database::{Transaction, sql};
use crate::deployment::Deployment;
use crate::device_settings::{self, DeviceSettingsError, WORK_OFFLINE};
use crate::events::{self, Answer, AnswerStatus, Event, EventKind, EventStatus};
use crate::posts::{self, PostError};

/// How far back from its cursor's time a pull starts, so a row written
/// in a transaction that committed after a later one is not missed.
pub const OVERLAP_SECONDS: i64 = 60;
pub const DEFAULT_LIMIT: usize = 200;
pub const MAX_LIMIT: usize = 500;

/// A place in the pull order: the server's write time and the id of a
/// row, the last one pulled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Cursor {
    pub last_synced_at: DateTime<Utc>,
    pub id: Uuid,
}

impl Cursor {
    /// Before every row.
    pub fn start() -> Cursor {
        Cursor {
            last_synced_at: DateTime::<Utc>::UNIX_EPOCH,
            id: Uuid::nil(),
        }
    }

    /// The row's place in the pull order, from its wire form.
    pub fn of_row(row: &TableRow) -> Option<Cursor> {
        let last_synced_at = row.row.get("last_synced_at")?.as_str()?;
        let id = row.row.get("id")?.as_str()?;
        Some(Cursor {
            last_synced_at: DateTime::parse_from_rfc3339(last_synced_at)
                .ok()?
                .with_timezone(&Utc),
            id: Uuid::parse_str(id).ok()?,
        })
    }
}

/// A row as it travels on the wire: its table, and the row as JSON.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableRow {
    pub table: String,
    pub row: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PushRequest {
    pub events: Vec<Event>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PushResponse {
    pub events: Vec<Answer>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PullRequest {
    /// The device's stored cursor, pulled from with the overlap.
    #[serde(default)]
    pub cursor: Option<Cursor>,
    /// The `next` of the page before, continued exactly.
    #[serde(default)]
    pub after: Option<Cursor>,
    #[serde(default)]
    pub limit: Option<usize>,
}

/// One page of a pull.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Page {
    pub rows: Vec<TableRow>,
    /// Where the next page starts, when this one was full.
    #[serde(default)]
    pub next: Option<Cursor>,
}

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Post(#[from] PostError),
    #[error(transparent)]
    DeviceSettings(#[from] DeviceSettingsError),
    /// The server could not be reached, or refused the request: the
    /// message is for the person, the detail inside for the log.
    #[error("Could not sync data")]
    Transport(String),
    #[error("the server answered with something unexpected: {0}")]
    Malformed(String),
}

/// Answers a push: applies the events in the order given, each in its own
/// transaction, through the resource of its table.
pub async fn push(
    deployment: &Deployment,
    request: PushRequest,
) -> Result<PushResponse, SyncError> {
    let mut answers = Vec::with_capacity(request.events.len());
    for event in &request.events {
        let answer = match event.table.as_str() {
            posts::TABLE => posts::apply_event(deployment, event).await?,
            _ => Answer::rejected(event.id, "unknown_table", None),
        };
        answers.push(answer);
    }
    Ok(PushResponse { events: answers })
}

/// Answers a pull: the rows the server wrote after the cursor, in cursor
/// order, deleted or not. One table today; with more, each table's rows
/// after the cursor are merged by their place in the order and cut at the
/// limit.
pub async fn pull(deployment: &Deployment, request: PullRequest) -> Result<Page, SyncError> {
    let limit = request.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let after = match (request.after, request.cursor) {
        (Some(after), _) => after,
        (None, Some(cursor)) => Cursor {
            last_synced_at: cursor.last_synced_at - Duration::seconds(OVERLAP_SECONDS),
            id: Uuid::nil(),
        },
        (None, None) => Cursor::start(),
    };

    let mut tx = deployment.begin().await?;
    let posts = posts::list_synced_after(&mut tx, &after, limit).await?;
    tx.commit().await?;

    let rows = posts
        .iter()
        .map(posts::table_row)
        .collect::<Result<Vec<TableRow>, PostError>>()?;
    let next = if rows.len() == limit {
        rows.last().and_then(Cursor::of_row)
    } else {
        None
    };
    Ok(Page { rows, next })
}

/// The server, as a device reaches it.
pub trait Transport {
    fn push(
        &self,
        request: PushRequest,
    ) -> impl Future<Output = Result<PushResponse, SyncError>> + Send;
    fn pull(&self, request: PullRequest) -> impl Future<Output = Result<Page, SyncError>> + Send;
}

/// The server in this process: its functions called directly, for tests.
pub struct InProcess<'a> {
    pub server: &'a Deployment,
}

impl Transport for InProcess<'_> {
    async fn push(&self, request: PushRequest) -> Result<PushResponse, SyncError> {
        push(self.server, request).await
    }

    async fn pull(&self, request: PullRequest) -> Result<Page, SyncError> {
        pull(self.server, request).await
    }
}

/// The server over HTTP, at its base URL, for the apps.
#[cfg(feature = "sync-http")]
pub struct Http {
    client: reqwest::Client,
    base_url: String,
}

#[cfg(feature = "sync-http")]
impl Http {
    pub fn new(base_url: &str) -> Result<Http, SyncError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|error| SyncError::Transport(error.to_string()))?;
        Ok(Http {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    async fn post<Request: Serialize, Response: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        request: &Request,
    ) -> Result<Response, SyncError> {
        let response = self
            .client
            .post(format!("{}{path}", self.base_url))
            .json(request)
            .send()
            .await
            .map_err(|error| SyncError::Transport(error.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(SyncError::Transport(format!(
                "the server refused {path} with {status}"
            )));
        }
        response
            .json()
            .await
            .map_err(|error| SyncError::Malformed(error.to_string()))
    }
}

#[cfg(feature = "sync-http")]
impl Transport for Http {
    async fn push(&self, request: PushRequest) -> Result<PushResponse, SyncError> {
        self.post("/api/sync/push", &request).await
    }

    async fn pull(&self, request: PullRequest) -> Result<Page, SyncError> {
        self.post("/api/sync/pull", &request).await
    }
}

/// What one sync did.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// The device is working offline: nothing was sent or asked for.
    pub offline: bool,
    /// Events the server applied.
    pub pushed: usize,
    /// Events that came back as conflicts.
    pub conflicts: usize,
    /// Events the server rejected.
    pub rejected: usize,
    /// Rows received by the pull.
    pub pulled: usize,
    /// Events still waiting after this sync.
    pub pending: i64,
    /// Why the push failed as a whole, when it did; the pull still ran.
    pub push_error: Option<String>,
}

/// Where a device stands: whether it works offline, what waits, what
/// conflicted, where the pull is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub offline: bool,
    pub pending: i64,
    pub conflicts: i64,
    pub cursor: Option<Cursor>,
}

/// One sync on a device: push the pending events and apply the answers,
/// then pull from the cursor. A push that fails as a whole changes
/// nothing on the device and the pull still runs. Working offline, it
/// makes no request and only reports what waits.
pub async fn run<T: Transport>(
    deployment: &Deployment,
    transport: &T,
) -> Result<Report, SyncError> {
    let mut report = Report::default();
    let before = status(deployment).await?;
    if before.offline {
        report.offline = true;
        report.pending = before.pending;
        return Ok(report);
    }
    if let Err(error) = push_pending(deployment, transport, &mut report).await {
        report.push_error = Some(error.to_string());
    }
    pull_pages(deployment, transport, &mut report).await?;
    report.pending = status(deployment).await?.pending;
    Ok(report)
}

pub async fn status(deployment: &Deployment) -> Result<Status, SyncError> {
    let mut tx = deployment.begin().await?;
    let status = Status {
        offline: device_settings::get(&mut tx, WORK_OFFLINE).await?,
        pending: events::count(&mut tx, EventStatus::Pending).await?,
        conflicts: events::count(&mut tx, EventStatus::Conflict).await?,
        cursor: sql!(&mut tx, sync_state::cursor())?,
    };
    tx.commit().await?;
    Ok(status)
}

/// Chooses to work offline, or not: a device setting, so it holds across
/// restarts. Writes keep queueing either way.
pub async fn set_offline(deployment: &Deployment, offline: bool) -> Result<(), SyncError> {
    let mut tx = deployment.begin().await?;
    device_settings::set(&mut tx, WORK_OFFLINE, &offline).await?;
    tx.commit().await?;
    Ok(())
}

async fn push_pending<T: Transport>(
    deployment: &Deployment,
    transport: &T,
    report: &mut Report,
) -> Result<(), SyncError> {
    let mut tx = deployment.begin().await?;
    let pending = events::pending(&mut tx).await?;
    tx.commit().await?;
    if pending.is_empty() {
        return Ok(());
    }

    let request = PushRequest {
        events: pending.iter().map(|stored| stored.event.clone()).collect(),
    };
    let response = transport.push(request).await?;

    let mut tx = deployment.begin().await?;
    let mut answered: BTreeMap<(String, Uuid), Option<TableRow>> = BTreeMap::new();
    for answer in response.events {
        let Some(stored) = pending.iter().find(|stored| stored.event.id == answer.id) else {
            continue;
        };
        let event = &stored.event;
        let key = (event.table.clone(), event.row_id);
        match answer.status {
            AnswerStatus::Applied => {
                events::delete(&mut tx, event.id).await?;
                report.pushed += 1;
                answered.insert(key, answer.server);
            }
            AnswerStatus::Conflict => {
                events::mark_conflict(&mut tx, event.id, &answer.conflicts).await?;
                report.conflicts += 1;
                answered.insert(key, answer.server);
            }
            AnswerStatus::Rejected => {
                report.rejected += 1;
                let reason = answer.reason.unwrap_or_else(|| "rejected".to_string());
                match (event.kind, answer.server) {
                    // The server's row wins
                    (_, Some(server)) => {
                        events::delete(&mut tx, event.id).await?;
                        answered.insert(key, Some(server));
                    }
                    // The server will never have this row: drop it
                    (EventKind::Create, None) => {
                        events::delete(&mut tx, event.id).await?;
                        events::delete_pending_of_row(&mut tx, &event.table, event.row_id).await?;
                        delete_row(&mut tx, &event.table, event.row_id).await?;
                        answered.remove(&key);
                    }
                    // Kept pending with the reason, for a debugging screen
                    (EventKind::Update, None) => {
                        events::mark_error(&mut tx, event.id, &reason).await?;
                    }
                }
            }
        }
    }
    for (_, server) in answered {
        if let Some(row) = server {
            apply_row(&mut tx, &row).await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

async fn pull_pages<T: Transport>(
    deployment: &Deployment,
    transport: &T,
    report: &mut Report,
) -> Result<(), SyncError> {
    let mut request = PullRequest {
        cursor: status(deployment).await?.cursor,
        after: None,
        limit: Some(DEFAULT_LIMIT),
    };
    loop {
        let page = transport.pull(request.clone()).await?;
        let mut tx = deployment.begin().await?;
        for row in &page.rows {
            apply_row(&mut tx, row).await?;
        }
        report.pulled += page.rows.len();
        if let Some(cursor) = page.rows.last().and_then(Cursor::of_row) {
            sql!(&mut tx, sync_state::save_cursor(&cursor))?;
        }
        tx.commit().await?;
        match page.next {
            Some(next) => {
                request = PullRequest {
                    cursor: None,
                    after: Some(next),
                    limit: Some(DEFAULT_LIMIT),
                }
            }
            None => return Ok(()),
        }
    }
}

/// Writes a server row over the device's, unless the device has an
/// event for it still pending; true when it was written.
async fn apply_row(tx: &mut Transaction, row: &TableRow) -> Result<bool, SyncError> {
    match row.table.as_str() {
        posts::TABLE => Ok(posts::apply_server_row(tx, &row.row).await?),
        other => Err(SyncError::Malformed(format!(
            "a row of an unknown table '{other}'"
        ))),
    }
}

async fn delete_row(tx: &mut Transaction, table: &str, id: Uuid) -> Result<(), SyncError> {
    match table {
        posts::TABLE => Ok(posts::delete_row(tx, id).await?),
        other => Err(SyncError::Malformed(format!(
            "a row of an unknown table '{other}'"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::database::{Database, with_each_database};
    use crate::deployment::Role;
    use crate::{conflicts, posts};

    /// A device as the app opens it: its settings set up.
    async fn device() -> Deployment {
        let database = Database::open_sqlite_in_memory()
            .await
            .expect("an in-memory SQLite database");
        let deployment = Deployment::new(database, Role::Device);
        device_settings::upgrade(&deployment)
            .await
            .expect("the settings set up");
        deployment
    }

    async fn sync(device: &Deployment, server: &Deployment) -> Report {
        run(device, &InProcess { server }).await.expect("a sync")
    }

    async fn bodies(deployment: &Deployment) -> Vec<String> {
        posts::list(deployment)
            .await
            .expect("the posts")
            .into_iter()
            .map(|post| post.body)
            .collect()
    }

    async fn pending(deployment: &Deployment) -> i64 {
        status(deployment).await.expect("the status").pending
    }

    #[tokio::test]
    async fn sync_a_post_written_offline_reaches_the_server_and_the_other_device() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let a = device().await;
            let b = device().await;
            let post = posts::create(&a, "from a").await.expect("a post");
            assert_eq!(pending(&a).await, 1);

            let report = sync(&a, &server).await;
            assert_eq!(report.pushed, 1);
            assert_eq!(report.pending, 0);
            let on_a = posts::get(&a, post.id).await.expect("the post on a");
            assert_eq!(on_a.version, 1);
            assert!(on_a.last_synced_at.is_some());
            assert_eq!(bodies(&server).await, vec!["from a"]);

            let report = sync(&b, &server).await;
            assert_eq!(report.pulled, 1);
            assert_eq!(posts::get(&b, post.id).await.expect("the post on b"), on_a);
            assert!(status(&b).await.expect("the status").cursor.is_some());
        })
        .await;
    }

    #[tokio::test]
    async fn sync_an_edit_from_the_servers_version_applies_everywhere() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let a = device().await;
            let b = device().await;
            let post = posts::create(&a, "first").await.expect("a post");
            sync(&a, &server).await;
            sync(&b, &server).await;

            posts::update(&b, post.id, "edited on b")
                .await
                .expect("edited");
            sync(&b, &server).await;
            sync(&a, &server).await;

            let on_a = posts::get(&a, post.id).await.expect("the post on a");
            assert_eq!(on_a.body, "edited on b");
            assert_eq!(on_a.version, 2);
            assert_eq!(bodies(&server).await, vec!["edited on b"]);
        })
        .await;
    }

    #[tokio::test]
    async fn sync_the_same_field_edited_on_two_devices_keeps_the_first_and_returns_a_conflict() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let a = device().await;
            let b = device().await;
            let post = posts::create(&a, "start").await.expect("a post");
            sync(&a, &server).await;
            sync(&b, &server).await;

            posts::update(&a, post.id, "a's edit")
                .await
                .expect("edited on a");
            posts::update(&b, post.id, "b's edit")
                .await
                .expect("edited on b");
            sync(&a, &server).await;
            let report = sync(&b, &server).await;
            assert_eq!(report.conflicts, 1);

            assert_eq!(bodies(&server).await, vec!["a's edit"]);
            let on_b = posts::get(&b, post.id).await.expect("the post on b");
            assert_eq!(on_b.body, "a's edit");
            assert_eq!(on_b.version, 2);
            let conflicts = conflicts::list(&b).await.expect("the conflicts");
            assert_eq!(conflicts.len(), 1);
            assert_eq!(conflicts[0].about.as_deref(), Some("a's edit"));
            assert_eq!(conflicts[0].fields[0].name, "body");
            assert_eq!(conflicts[0].fields[0].yours, json!("b's edit"));
            assert_eq!(conflicts[0].fields[0].now, json!("a's edit"));
            assert_eq!(status(&b).await.expect("the status").conflicts, 1);
        })
        .await;
    }

    #[tokio::test]
    async fn sync_different_fields_edited_on_two_devices_both_apply() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let a = device().await;
            let b = device().await;
            let post = posts::create(&a, "start").await.expect("a post");
            sync(&a, &server).await;
            sync(&b, &server).await;

            posts::update(&a, post.id, "a's edit")
                .await
                .expect("edited on a");
            posts::delete(&b, post.id).await.expect("deleted on b");
            sync(&a, &server).await;
            let report = sync(&b, &server).await;
            assert_eq!(report.conflicts, 0);
            assert_eq!(report.pushed, 1);

            assert!(bodies(&server).await.is_empty());
            let mut tx = server.begin().await.expect("a transaction");
            let row = posts::get_in(&mut tx, post.id)
                .await
                .expect("the row")
                .expect("kept");
            assert_eq!(row.body, "a's edit");
            assert!(row.deleted_at.is_some());
            assert_eq!(row.version, 3);
        })
        .await;
    }

    #[tokio::test]
    async fn sync_two_edits_made_offline_on_one_device_do_not_conflict_with_each_other() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let a = device().await;
            let post = posts::create(&a, "one").await.expect("a post");
            posts::update(&a, post.id, "two").await.expect("edited");
            posts::update(&a, post.id, "three")
                .await
                .expect("edited again");
            assert_eq!(pending(&a).await, 3);

            let report = sync(&a, &server).await;
            assert_eq!(report.pushed, 3);
            assert_eq!(report.conflicts, 0);
            assert_eq!(bodies(&server).await, vec!["three"]);
            assert_eq!(posts::get(&a, post.id).await.expect("the post").version, 3);
        })
        .await;
    }

    /// A server whose push answers never reach the device the first time.
    struct PushLost<'a> {
        server: &'a Deployment,
        lost: std::sync::Mutex<bool>,
    }

    impl Transport for PushLost<'_> {
        async fn push(&self, request: PushRequest) -> Result<PushResponse, SyncError> {
            let response = push(self.server, request).await?;
            let mut lost = self.lost.lock().expect("the lock");
            if *lost {
                *lost = false;
                return Err(SyncError::Transport("the connection dropped".to_string()));
            }
            Ok(response)
        }

        async fn pull(&self, request: PullRequest) -> Result<Page, SyncError> {
            pull(self.server, request).await
        }
    }

    #[tokio::test]
    async fn sync_a_retried_event_gets_the_same_answer_and_is_not_applied_twice() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let a = device().await;
            let post = posts::create(&a, "start").await.expect("a post");
            sync(&a, &server).await;
            posts::update(&a, post.id, "edited").await.expect("edited");
            let transport = PushLost {
                server: &server,
                lost: std::sync::Mutex::new(true),
            };

            let report = run(&a, &transport).await.expect("a sync");
            assert!(report.push_error.is_some());
            assert_eq!(report.pending, 1);
            assert_eq!(
                posts::get(&a, post.id).await.expect("the post").body,
                "edited"
            );

            let report = run(&a, &transport).await.expect("a sync");
            assert_eq!(report.pushed, 1);
            assert_eq!(report.pending, 0);
            let on_server = posts::get(&server, post.id).await.expect("the post");
            assert_eq!(on_server.version, 2);
            assert_eq!(posts::get(&a, post.id).await.expect("the post"), on_server);
        })
        .await;
    }

    #[tokio::test]
    async fn sync_a_pulled_row_never_replaces_a_row_with_a_pending_event() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let a = device().await;
            let b = device().await;
            let post = posts::create(&a, "start").await.expect("a post");
            sync(&a, &server).await;
            sync(&b, &server).await;
            posts::update(&a, post.id, "a's edit")
                .await
                .expect("edited on a");
            sync(&a, &server).await;

            posts::update(&b, post.id, "b's edit, offline")
                .await
                .expect("edited on b");
            let transport = PushLost {
                server: &server,
                lost: std::sync::Mutex::new(true),
            };
            let report = run(&b, &transport).await.expect("a sync");
            assert!(report.push_error.is_some());
            assert_eq!(report.pulled, 1);
            assert_eq!(
                posts::get(&b, post.id).await.expect("the post").body,
                "b's edit, offline"
            );
        })
        .await;
    }

    /// A server on which the device makes another edit while its push is
    /// in flight.
    struct EditDuringPush<'a> {
        server: &'a Deployment,
        device: &'a Deployment,
        post_id: Uuid,
    }

    impl Transport for EditDuringPush<'_> {
        async fn push(&self, request: PushRequest) -> Result<PushResponse, SyncError> {
            let response = push(self.server, request).await?;
            posts::update(self.device, self.post_id, "edited during the push").await?;
            Ok(response)
        }

        async fn pull(&self, request: PullRequest) -> Result<Page, SyncError> {
            pull(self.server, request).await
        }
    }

    #[tokio::test]
    async fn sync_an_event_queued_during_a_push_stays_pending_and_keeps_its_row() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let a = device().await;
            let post = posts::create(&a, "start").await.expect("a post");
            sync(&a, &server).await;
            posts::update(&a, post.id, "first edit")
                .await
                .expect("edited");
            let transport = EditDuringPush {
                server: &server,
                device: &a,
                post_id: post.id,
            };

            let report = run(&a, &transport).await.expect("a sync");
            assert_eq!(report.pushed, 1);
            assert_eq!(report.pending, 1);
            let on_a = posts::get(&a, post.id).await.expect("the post");
            assert_eq!(on_a.body, "edited during the push");
            assert_eq!(on_a.version, 1);

            let report = sync(&a, &server).await;
            assert_eq!(report.pushed, 1);
            assert_eq!(report.conflicts, 0);
            assert_eq!(bodies(&server).await, vec!["edited during the push"]);
            assert_eq!(posts::get(&a, post.id).await.expect("the post").version, 3);
        })
        .await;
    }

    #[tokio::test]
    async fn sync_a_conflict_reapplied_reaches_both_devices_and_one_let_go_sends_nothing() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let a = device().await;
            let b = device().await;
            let post = posts::create(&a, "start").await.expect("a post");
            sync(&a, &server).await;
            sync(&b, &server).await;
            posts::update(&a, post.id, "a's edit")
                .await
                .expect("edited on a");
            posts::update(&b, post.id, "b's edit")
                .await
                .expect("edited on b");
            sync(&a, &server).await;
            sync(&b, &server).await;
            let conflict = conflicts::list(&b).await.expect("the conflicts").remove(0);

            conflicts::reapply(&b, conflict.id)
                .await
                .expect("reapplied");
            assert!(conflicts::list(&b).await.expect("the conflicts").is_empty());
            assert_eq!(
                posts::get(&b, post.id).await.expect("the post").body,
                "b's edit"
            );
            let report = sync(&b, &server).await;
            assert_eq!(report.pushed, 1);
            sync(&a, &server).await;
            assert_eq!(bodies(&server).await, vec!["b's edit"]);
            assert_eq!(bodies(&a).await, vec!["b's edit"]);

            posts::update(&a, post.id, "a again")
                .await
                .expect("edited on a");
            posts::update(&b, post.id, "b again")
                .await
                .expect("edited on b");
            sync(&a, &server).await;
            sync(&b, &server).await;
            let conflict = conflicts::list(&b).await.expect("the conflicts").remove(0);
            conflicts::let_go(&b, conflict.id).await.expect("let go");
            let report = sync(&b, &server).await;
            assert_eq!(report.pushed, 0);
            assert_eq!(bodies(&server).await, vec!["a again"]);
            assert_eq!(bodies(&b).await, vec!["a again"]);
            let error = conflicts::let_go(&b, conflict.id).await.expect_err("gone");
            assert!(matches!(error, conflicts::ConflictError::NotFound { .. }));
        })
        .await;
    }

    #[tokio::test]
    async fn sync_pull_pages_in_cursor_order_and_resends_the_overlap() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let first = posts::create(&server, "first").await.expect("a post");
            let second = posts::create(&server, "second").await.expect("a post");
            let third = posts::create(&server, "third").await.expect("a post");

            let page = pull(
                &server,
                PullRequest {
                    cursor: None,
                    after: None,
                    limit: Some(2),
                },
            )
            .await
            .expect("a page");
            assert_eq!(page.rows.len(), 2);
            assert_eq!(page.rows[0].row["id"], json!(first.id));
            assert_eq!(page.rows[1].row["id"], json!(second.id));
            let next = page.next.expect("another page");
            assert_eq!(next.id, second.id);

            let page = pull(
                &server,
                PullRequest {
                    cursor: None,
                    after: Some(next),
                    limit: Some(2),
                },
            )
            .await
            .expect("a page");
            assert_eq!(page.rows.len(), 1);
            assert_eq!(page.rows[0].row["id"], json!(third.id));
            assert!(page.next.is_none());

            let cursor = Cursor::of_row(&page.rows[0]).expect("a cursor");
            let page = pull(
                &server,
                PullRequest {
                    cursor: Some(cursor),
                    after: None,
                    limit: None,
                },
            )
            .await
            .expect("a page");
            assert_eq!(
                page.rows.len(),
                3,
                "the overlap sends the last minute again"
            );
        })
        .await;
    }

    /// A server that must not be reached.
    struct Unreachable;

    impl Transport for Unreachable {
        async fn push(&self, _request: PushRequest) -> Result<PushResponse, SyncError> {
            panic!("the device is working offline and must not push")
        }

        async fn pull(&self, _request: PullRequest) -> Result<Page, SyncError> {
            panic!("the device is working offline and must not pull")
        }
    }

    #[tokio::test]
    async fn sync_working_offline_makes_no_request_and_holds_until_chosen_otherwise() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let a = device().await;
            set_offline(&a, true).await.expect("offline");
            assert!(status(&a).await.expect("the status").offline);
            posts::create(&a, "written offline").await.expect("a post");

            let report = run(&a, &Unreachable).await.expect("a sync");
            assert!(report.offline);
            assert_eq!(report.pending, 1);
            assert!(report.push_error.is_none());
            assert!(bodies(&server).await.is_empty());

            set_offline(&a, false).await.expect("online");
            let report = sync(&a, &server).await;
            assert!(!report.offline);
            assert_eq!(report.pushed, 1);
            assert_eq!(bodies(&server).await, vec!["written offline"]);
            let status = status(&a).await.expect("the status");
            assert!(!status.offline);
            assert!(status.cursor.is_some());
        })
        .await;
    }

    #[tokio::test]
    async fn sync_a_delete_on_the_server_hides_the_post_on_the_device() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let a = device().await;
            let post = posts::create(&server, "on the web").await.expect("a post");
            sync(&a, &server).await;
            assert_eq!(bodies(&a).await, vec!["on the web"]);

            posts::delete(&server, post.id).await.expect("deleted");
            sync(&a, &server).await;
            assert!(bodies(&a).await.is_empty());
        })
        .await;
    }

    #[tokio::test]
    async fn sync_push_rejects_an_event_of_an_unknown_table() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let event = Event::create("app_nothing", Uuid::now_v7(), events::Fields::new(), now());
            let response = push(
                &server,
                PushRequest {
                    events: vec![event],
                },
            )
            .await
            .expect("answered");
            assert_eq!(response.events[0].status, AnswerStatus::Rejected);
            assert_eq!(response.events[0].reason.as_deref(), Some("unknown_table"));
        })
        .await;
    }

    #[tokio::test]
    async fn sync_a_rejected_create_drops_the_devices_row() {
        with_each_database(|database| async move {
            let server = Deployment::new(database, Role::Server);
            let a = device().await;
            let post = posts::create(&a, "fine").await.expect("a post");
            {
                // Rewrite the queued create to one the server refuses
                let mut tx = a.begin().await.expect("a transaction");
                let stored = events::pending(&mut tx)
                    .await
                    .expect("the events")
                    .remove(0);
                events::delete(&mut tx, stored.event.id)
                    .await
                    .expect("deleted");
                let mut broken = stored.event.clone();
                broken.changes = fields(json!({ "body": "x", "version": 5 }));
                events::queue(&mut tx, &broken).await.expect("queued");
                tx.commit().await.expect("committed");
            }

            let report = sync(&a, &server).await;
            assert_eq!(report.rejected, 1);
            assert_eq!(report.pending, 0);
            assert!(bodies(&a).await.is_empty());
            assert!(bodies(&server).await.is_empty());
            let error = posts::get(&a, post.id).await.expect_err("dropped");
            assert!(matches!(error, PostError::NotFound { .. }));
        })
        .await;
    }

    fn fields(value: Value) -> events::Fields {
        match value {
            Value::Object(fields) => fields,
            _ => panic!("fields are an object"),
        }
    }

    use crate::deployment::now;

    /// Two devices against a running web server, `SYNC_SERVER_URL`:
    ///
    /// ```sh
    /// SYNC_SERVER_URL=http://localhost:4010 cargo test -p app_core --features sync-http two_devices -- --ignored
    /// ```
    #[cfg(feature = "sync-http")]
    #[tokio::test]
    #[ignore]
    async fn sync_two_devices_over_http() {
        let url = std::env::var("SYNC_SERVER_URL").expect("SYNC_SERVER_URL");
        let server = Http::new(&url).expect("a transport");
        let a = device().await;
        let b = device().await;
        let post = posts::create(&a, "over http from a").await.expect("a post");
        run(&a, &server).await.expect("a sync");
        run(&b, &server).await.expect("a sync");
        assert_eq!(
            posts::get(&b, post.id).await.expect("the post on b").body,
            "over http from a"
        );

        posts::update(&a, post.id, "a's edit")
            .await
            .expect("edited on a");
        posts::update(&b, post.id, "b's edit")
            .await
            .expect("edited on b");
        run(&a, &server).await.expect("a sync");
        let report = run(&b, &server).await.expect("a sync");
        assert_eq!(report.conflicts, 1);
        assert_eq!(
            posts::get(&b, post.id).await.expect("the post on b").body,
            "a's edit"
        );
    }
}
