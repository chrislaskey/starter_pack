//! Conflicts: the changes a device made that the server did not keep,
//! because someone else changed the same field first, kept until the
//! person reapplies or lets go of each, see "Conflicts" in
//! `docs/design/offline-sync.md`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::deployment::{Deployment, now};
use crate::events::{self, AnswerStatus, Event, EventStatus, Fields, StoredEvent};
use crate::posts::{self, Post, PostError};

/// A conflicted change as the page shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    pub id: Uuid,
    pub table: String,
    pub row_id: Uuid,
    /// When the change was made, on the device's clock.
    pub made_at: DateTime<Utc>,
    /// What it was about, the post's first line; none when the post is gone.
    pub about: Option<String>,
    pub fields: Vec<ConflictField>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictField {
    pub name: String,
    /// The person's value.
    pub yours: Value,
    /// The value on this device now, the server's as of the last sync.
    pub now: Value,
}

#[derive(Debug, Error)]
pub enum ConflictError {
    #[error("conflict {id} not found")]
    NotFound { id: Uuid },
    #[error("the post is gone; the change can only be let go")]
    Gone,
    #[error(transparent)]
    Post(#[from] PostError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Every conflict, newest first.
pub async fn list(deployment: &Deployment) -> Result<Vec<Conflict>, ConflictError> {
    let mut tx = deployment.begin().await?;
    let mut conflicts = Vec::new();
    for stored in events::conflicts(&mut tx).await? {
        let post = posts::get_in(&mut tx, stored.event.row_id).await?;
        conflicts.push(conflict(&stored, post.as_ref())?);
    }
    tx.commit().await?;
    Ok(conflicts)
}

/// Queues the person's values again as a new change from the row as it
/// stands now, and drops the conflict.
pub async fn reapply(deployment: &Deployment, id: Uuid) -> Result<(), ConflictError> {
    let mut tx = deployment.begin().await?;
    let stored = get(&mut tx, id).await?;
    let Some(post) = posts::get_in(&mut tx, stored.event.row_id).await? else {
        return Err(ConflictError::Gone);
    };
    let names: Vec<&str> = stored.conflicts.keys().map(String::as_str).collect();
    let base = post.fields(&names)?;
    let changes: Fields = stored
        .conflicts
        .iter()
        .map(|(name, conflict)| (name.clone(), conflict.yours.clone()))
        .collect();
    let event = Event::update(
        &stored.event.table,
        post.id,
        post.version,
        base,
        changes,
        now(),
    );
    let applied = posts::apply_in(&mut tx, deployment.role, &event).await?;
    if applied.answer.status == AnswerStatus::Rejected {
        return Err(PostError::Rejected(applied.answer.reason.unwrap_or_default()).into());
    }
    events::delete(&mut tx, id).await?;
    tx.commit().await?;
    Ok(())
}

/// Drops the conflict; nothing is sent.
pub async fn let_go(deployment: &Deployment, id: Uuid) -> Result<(), ConflictError> {
    let mut tx = deployment.begin().await?;
    get(&mut tx, id).await?;
    events::delete(&mut tx, id).await?;
    tx.commit().await?;
    Ok(())
}

async fn get(
    tx: &mut crate::database::Transaction,
    id: Uuid,
) -> Result<StoredEvent, ConflictError> {
    match events::get(tx, id).await? {
        Some(stored) if stored.status == EventStatus::Conflict => Ok(stored),
        _ => Err(ConflictError::NotFound { id }),
    }
}

fn conflict(stored: &StoredEvent, post: Option<&Post>) -> Result<Conflict, ConflictError> {
    let now = match post {
        Some(post) => post.fields(&["body", "deleted_at"])?,
        None => Fields::new(),
    };
    Ok(Conflict {
        id: stored.event.id,
        table: stored.event.table.clone(),
        row_id: stored.event.row_id,
        made_at: stored.event.made_at,
        about: post
            .filter(|post| post.deleted_at.is_none())
            .map(|post| post.body.lines().next().unwrap_or_default().to_string()),
        fields: stored
            .conflicts
            .iter()
            .map(|(name, field)| ConflictField {
                name: name.clone(),
                yours: field.yours.clone(),
                now: now.get(name).cloned().unwrap_or(Value::Null),
            })
            .collect(),
    })
}
