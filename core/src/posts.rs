//! Posts: the first resource. A post is some text, written, edited and
//! deleted from any deployment. Called through the module, `posts::list`,
//! as the standard library's `fs::read` is.
//!
//! Every write is an event applied by `apply_event`, on the server at
//! once and on a device at once too, where it is queued and the server
//! answers it on the next push with the same function, see `events`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

use crate::database::{Transaction, sql};
use crate::deployment::{Deployment, Role, now};
use crate::events::{self, Answer, AnswerStatus, Conflicts, Event, EventKind, Fields};
use crate::sync::{Cursor, TableRow};

pub const TABLE: &str = "app_posts";
/// The fields an event may set when it creates a post.
const CREATE_FIELDS: &[&str] = &["body"];
/// The fields an event may change on a post.
const UPDATE_FIELDS: &[&str] = &["body", "deleted_at"];

/// A post as the pages show it, with the sync columns every synced row
/// carries, see `docs/design/offline-sync.md`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Post {
    pub id: Uuid,
    pub body: String,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// The server's version, 0 on a device's post not yet pushed.
    pub version: i64,
    /// When the server last wrote it; none on a device's post not yet pushed.
    pub last_synced_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Error)]
pub enum PostError {
    #[error("post {id} not found")]
    NotFound { id: Uuid },
    #[error("a post needs some text")]
    BlankBody,
    #[error("{0}")]
    Invalid(String),
    #[error("the change was refused: {0}")]
    Rejected(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<serde_json::Error> for PostError {
    fn from(error: serde_json::Error) -> PostError {
        PostError::Invalid(error.to_string())
    }
}

/// Every post not deleted, newest first.
pub async fn list(deployment: &Deployment) -> Result<Vec<Post>, PostError> {
    let mut tx = deployment.begin().await?;
    let posts = sql!(&mut tx, posts::list())?;
    tx.commit().await?;
    Ok(posts)
}

/// One post by its id, unless deleted.
pub async fn get(deployment: &Deployment, id: Uuid) -> Result<Post, PostError> {
    let mut tx = deployment.begin().await?;
    let post = get_live(&mut tx, id).await?;
    tx.commit().await?;
    Ok(post)
}

/// Writes a new post, with an id and the time from this deployment.
pub async fn create(deployment: &Deployment, body: &str) -> Result<Post, PostError> {
    let body = checked_body(body)?;
    let event = Event::create(
        TABLE,
        Uuid::now_v7(),
        fields(json!({ "body": body })),
        now(),
    );
    let mut tx = deployment.begin().await?;
    let applied = apply_in(&mut tx, deployment.role, &event).await?;
    tx.commit().await?;
    applied.post()
}

/// Writes a new body over the post, with the time from this deployment.
pub async fn update(deployment: &Deployment, id: Uuid, body: &str) -> Result<Post, PostError> {
    let body = checked_body(body)?;
    let mut tx = deployment.begin().await?;
    let post = get_live(&mut tx, id).await?;
    if post.body == body {
        tx.commit().await?;
        return Ok(post);
    }
    let event = Event::update(
        TABLE,
        id,
        post.version,
        fields(json!({ "body": post.body })),
        fields(json!({ "body": body })),
        now(),
    );
    let applied = apply_in(&mut tx, deployment.role, &event).await?;
    tx.commit().await?;
    applied.post()
}

/// Deletes the post: a soft delete, so it syncs like any edit.
pub async fn delete(deployment: &Deployment, id: Uuid) -> Result<(), PostError> {
    let mut tx = deployment.begin().await?;
    let post = get_live(&mut tx, id).await?;
    let event = Event::update(
        TABLE,
        id,
        post.version,
        fields(json!({ "deleted_at": null })),
        fields(json!({ "deleted_at": now() })),
        now(),
    );
    let applied = apply_in(&mut tx, deployment.role, &event).await?;
    tx.commit().await?;
    applied.post().map(|_| ())
}

/// Applies an event to its post and answers it: what the server does
/// with a pushed event, and what a local write does on any deployment.
pub async fn apply_event(deployment: &Deployment, event: &Event) -> Result<Answer, PostError> {
    let mut tx = deployment.begin().await?;
    let applied = apply_in(&mut tx, deployment.role, event).await?;
    tx.commit().await?;
    Ok(applied.answer)
}

/// The post as it travels on the wire.
pub fn table_row(post: &Post) -> Result<TableRow, PostError> {
    Ok(TableRow {
        table: TABLE.to_string(),
        row: serde_json::to_value(post)?,
    })
}

/// Writes the server's row over the device's, unless an event of the
/// device's for it is still pending; true when it was written.
pub(crate) async fn apply_server_row(tx: &mut Transaction, row: &Value) -> Result<bool, PostError> {
    let post: Post = serde_json::from_value(row.clone())?;
    if events::has_pending(tx, TABLE, post.id).await? {
        return Ok(false);
    }
    sql!(&mut *tx, posts::upsert(&post))?;
    Ok(true)
}

/// Removes a device's post whose create the server refused.
pub(crate) async fn delete_row(tx: &mut Transaction, id: Uuid) -> Result<(), PostError> {
    Ok(sql!(tx, posts::delete_row(id))?)
}

/// A page of the rows the server wrote after the cursor, deleted or not.
pub(crate) async fn list_synced_after(
    tx: &mut Transaction,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<Post>, PostError> {
    Ok(sql!(tx, posts::list_synced_after(after, limit))?)
}

/// The row by its id, deleted or not.
pub(crate) async fn get_in(tx: &mut Transaction, id: Uuid) -> Result<Option<Post>, PostError> {
    Ok(sql!(tx, posts::get(id))?)
}

async fn get_live(tx: &mut Transaction, id: Uuid) -> Result<Post, PostError> {
    match get_in(tx, id).await? {
        Some(post) if post.deleted_at.is_none() => Ok(post),
        _ => Err(PostError::NotFound { id }),
    }
}

/// An event applied: the answer, and the post as it now stands.
pub(crate) struct Applied {
    pub answer: Answer,
    pub post: Option<Post>,
}

impl Applied {
    fn post(self) -> Result<Post, PostError> {
        match (self.answer.status, self.post) {
            (AnswerStatus::Rejected, _) => Err(PostError::Rejected(
                self.answer.reason.unwrap_or_else(|| "rejected".to_string()),
            )),
            (_, Some(post)) => Ok(post),
            (_, None) => Err(PostError::NotFound { id: self.answer.id }),
        }
    }
}

/// Applies the event in the transaction: checks the fields it changes,
/// merges it into the row, writes the row, and then the role's part, the
/// record of the answer on the server or the queue on a device.
pub(crate) async fn apply_in(
    tx: &mut Transaction,
    role: Role,
    event: &Event,
) -> Result<Applied, PostError> {
    if role == Role::Server
        && let Some(recorded) = events::get(tx, event.id).await?
    {
        let post = get_in(tx, event.row_id).await?;
        let server = post.as_ref().map(table_row).transpose()?;
        return Ok(Applied {
            answer: recorded.answer(server),
            post,
        });
    }

    let allowed = match event.kind {
        EventKind::Create => CREATE_FIELDS,
        EventKind::Update => UPDATE_FIELDS,
    };
    if let Some(reason) = events::not_editable(&event.changes, allowed) {
        let post = get_in(tx, event.row_id).await?;
        return rejected(event, reason, post);
    }

    let now = now();
    let post = match event.kind {
        EventKind::Create => {
            if let Some(existing) = get_in(tx, event.row_id).await? {
                return rejected(event, "exists", Some(existing));
            }
            let (version, last_synced_at) = match role {
                Role::Server => (1, Some(now)),
                Role::Device => (0, None),
            };
            let blank = Post {
                id: event.row_id,
                body: String::new(),
                inserted_at: now,
                updated_at: now,
                version,
                last_synced_at,
                deleted_at: None,
            };
            let post = match blank.with_fields(&event.changes) {
                Ok(post) => post,
                Err(error) => return rejected(event, format!("invalid: {error}"), None),
            };
            sql!(&mut *tx, posts::insert(&post))?;
            finish(tx, role, event, Conflicts::new(), post).await?
        }
        EventKind::Update => {
            let Some(post) = get_in(tx, event.row_id).await? else {
                return rejected(event, "unknown_post", None);
            };
            let (changes, conflicts) = events::merge(
                &post.fields(UPDATE_FIELDS)?,
                post.version,
                event.base_version,
                &event.base,
                &event.changes,
            );
            let post = if changes.is_empty() {
                post
            } else {
                let mut updated = match post.with_fields(&changes) {
                    Ok(updated) => updated,
                    Err(error) => return rejected(event, format!("invalid: {error}"), Some(post)),
                };
                updated.updated_at = now;
                if role == Role::Server {
                    updated.version += 1;
                    updated.last_synced_at = Some(now);
                }
                sql!(&mut *tx, posts::update(&updated))?;
                updated
            };
            finish(tx, role, event, conflicts, post).await?
        }
    };
    Ok(post)
}

/// Answers an applied event and does the role's part.
async fn finish(
    tx: &mut Transaction,
    role: Role,
    event: &Event,
    conflicts: Conflicts,
    post: Post,
) -> Result<Applied, PostError> {
    let answer = Answer::applied(event.id, post.version, conflicts, table_row(&post)?);
    match role {
        Role::Server => events::record(tx, event, &answer).await?,
        Role::Device => events::queue(tx, event).await?,
    }
    Ok(Applied {
        answer,
        post: Some(post),
    })
}

fn rejected(
    event: &Event,
    reason: impl Into<String>,
    post: Option<Post>,
) -> Result<Applied, PostError> {
    let server = post.as_ref().map(table_row).transpose()?;
    Ok(Applied {
        answer: Answer::rejected(event.id, reason, server),
        post,
    })
}

impl Post {
    /// The named fields as they travel on the wire.
    pub(crate) fn fields(&self, names: &[&str]) -> Result<Fields, PostError> {
        let mut all = fields(serde_json::to_value(self)?);
        all.retain(|name, _| names.contains(&name.as_str()));
        Ok(all)
    }

    /// The post with the fields written over it, checked.
    fn with_fields(&self, changes: &Fields) -> Result<Post, PostError> {
        let mut all = fields(serde_json::to_value(self)?);
        for (name, value) in changes {
            all.insert(name.clone(), value.clone());
        }
        let post: Post = serde_json::from_value(Value::Object(all))?;
        Ok(Post {
            body: checked_body(&post.body)?,
            ..post
        })
    }
}

/// The fields of a JSON object, or none for anything else.
fn fields(value: Value) -> Fields {
    match value {
        Value::Object(fields) => fields,
        _ => Fields::new(),
    }
}

/// The body as it is kept: trimmed, and not blank.
fn checked_body(body: &str) -> Result<String, PostError> {
    let body = body.trim();
    if body.is_empty() {
        return Err(PostError::BlankBody);
    }
    Ok(body.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{Database, with_each_database};
    use crate::events::EventStatus;

    fn server(database: Database) -> Deployment {
        Deployment::new(database, Role::Server)
    }

    async fn device() -> Deployment {
        let database = Database::open_sqlite_in_memory()
            .await
            .expect("an in-memory SQLite database");
        Deployment::new(database, Role::Device)
    }

    async fn events_with_status(deployment: &Deployment, status: EventStatus) -> i64 {
        let mut tx = deployment.begin().await.expect("a transaction");
        events::count(&mut tx, status).await.expect("a count")
    }

    #[tokio::test]
    async fn posts_list_newest_first() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let first = create(&deployment, "first").await.expect("a post");
            let second = create(&deployment, "second").await.expect("a post");

            let posts = list(&deployment).await.expect("the posts");
            assert_eq!(posts, vec![second, first]);
        })
        .await;
    }

    #[tokio::test]
    async fn posts_get_by_id() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let written = create(&deployment, "a post").await.expect("a post");

            let post = get(&deployment, written.id).await.expect("the post");
            assert_eq!(post, written);
        })
        .await;
    }

    #[tokio::test]
    async fn posts_get_missing() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let id = Uuid::now_v7();

            let error = get(&deployment, id).await.expect_err("no such post");
            assert!(matches!(error, PostError::NotFound { id: missing } if missing == id));
        })
        .await;
    }

    #[tokio::test]
    async fn posts_create_trims_and_refuses_blank() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let post = create(&deployment, "  spaced  ").await.expect("a post");
            assert_eq!(post.body, "spaced");

            let error = create(&deployment, " \n ").await.expect_err("blank");
            assert!(matches!(error, PostError::BlankBody));
            assert_eq!(list(&deployment).await.expect("the posts").len(), 1);
        })
        .await;
    }

    #[tokio::test]
    async fn posts_create_on_the_server_is_version_one_and_logged() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let post = create(&deployment, "a post").await.expect("a post");
            assert_eq!(post.version, 1);
            assert_eq!(post.last_synced_at, Some(post.updated_at));
            assert_eq!(
                events_with_status(&deployment, EventStatus::Applied).await,
                1
            );
            assert_eq!(
                events_with_status(&deployment, EventStatus::Pending).await,
                0
            );
        })
        .await;
    }

    #[tokio::test]
    async fn posts_create_on_a_device_is_version_zero_and_queued() {
        let deployment = device().await;
        let post = create(&deployment, "a post").await.expect("a post");
        assert_eq!(post.version, 0);
        assert_eq!(post.last_synced_at, None);
        assert_eq!(
            events_with_status(&deployment, EventStatus::Pending).await,
            1
        );
        assert_eq!(
            events_with_status(&deployment, EventStatus::Applied).await,
            0
        );
    }

    #[tokio::test]
    async fn posts_update() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let written = create(&deployment, "before").await.expect("a post");

            let updated = update(&deployment, written.id, "after")
                .await
                .expect("updated");
            assert_eq!(updated.body, "after");
            assert_eq!(updated.inserted_at, written.inserted_at);
            assert!(updated.updated_at >= written.updated_at);
            assert_eq!(updated.version, 2);
            assert_eq!(
                get(&deployment, written.id).await.expect("the post"),
                updated
            );

            let same = update(&deployment, written.id, "after")
                .await
                .expect("unchanged");
            assert_eq!(same.version, 2);

            let error = update(&deployment, written.id, "")
                .await
                .expect_err("blank");
            assert!(matches!(error, PostError::BlankBody));
            let error = update(&deployment, Uuid::now_v7(), "x")
                .await
                .expect_err("missing");
            assert!(matches!(error, PostError::NotFound { .. }));
        })
        .await;
    }

    #[tokio::test]
    async fn posts_delete_hides_the_post() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let written = create(&deployment, "going").await.expect("a post");

            delete(&deployment, written.id).await.expect("deleted");
            let error = get(&deployment, written.id).await.expect_err("gone");
            assert!(matches!(error, PostError::NotFound { .. }));
            assert!(list(&deployment).await.expect("the posts").is_empty());
            let error = delete(&deployment, written.id)
                .await
                .expect_err("already gone");
            assert!(matches!(error, PostError::NotFound { .. }));

            let mut tx = deployment.begin().await.expect("a transaction");
            let row = get_in(&mut tx, written.id)
                .await
                .expect("the row")
                .expect("kept");
            assert!(row.deleted_at.is_some());
            assert_eq!(row.version, 2);
        })
        .await;
    }

    #[tokio::test]
    async fn posts_apply_event_answers_a_retry_the_same_without_applying_twice() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let post = create(&deployment, "before").await.expect("a post");
            let event = Event::update(
                TABLE,
                post.id,
                1,
                fields(json!({ "body": "before" })),
                fields(json!({ "body": "after" })),
                now(),
            );

            let first = apply_event(&deployment, &event).await.expect("applied");
            let again = apply_event(&deployment, &event)
                .await
                .expect("answered again");
            assert_eq!(first.status, AnswerStatus::Applied);
            assert_eq!(again.version, first.version);
            assert_eq!(
                get(&deployment, post.id).await.expect("the post").version,
                2
            );
        })
        .await;
    }

    #[tokio::test]
    async fn posts_apply_event_refuses_a_field_it_may_not_change() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let post = create(&deployment, "a post").await.expect("a post");
            let event = Event::update(
                TABLE,
                post.id,
                1,
                fields(json!({})),
                fields(json!({ "version": 9, "body": "x" })),
                now(),
            );

            let answer = apply_event(&deployment, &event).await.expect("answered");
            assert_eq!(answer.status, AnswerStatus::Rejected);
            assert_eq!(answer.reason.as_deref(), Some("not_editable: version"));
            assert_eq!(
                get(&deployment, post.id).await.expect("the post").body,
                "a post"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn posts_apply_event_refuses_an_invalid_create() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let event = Event::create(
                TABLE,
                Uuid::now_v7(),
                fields(json!({ "body": "  " })),
                now(),
            );

            let answer = apply_event(&deployment, &event).await.expect("answered");
            assert_eq!(answer.status, AnswerStatus::Rejected);
            assert_eq!(
                answer.reason.as_deref(),
                Some("invalid: a post needs some text")
            );
            assert!(list(&deployment).await.expect("the posts").is_empty());
        })
        .await;
    }
}
