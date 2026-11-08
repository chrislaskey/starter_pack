//! Events: one change a person made to a row, the only way a synced row
//! changes, see "Events" in `docs/design/offline-sync.md`. A resource
//! builds one for each save and applies it with the merge here; the
//! server answers a pushed event the same way, since both run the
//! resource's `apply_event`. The `app_events` table is the server's log
//! of every event answered and a device's queue of events not yet
//! answered, with the conflicts the person has not resolved.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::database::{Transaction, sql};
use crate::sync::TableRow;

/// A row's fields as they travel on the wire, by name.
pub type Fields = Map<String, Value>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventKind {
    Create,
    Update,
}

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EventKind::Create => "create",
            EventKind::Update => "update",
        }
    }

    pub fn parse(text: &str) -> Option<EventKind> {
        match text {
            "create" => Some(EventKind::Create),
            "update" => Some(EventKind::Update),
            _ => None,
        }
    }
}

/// One change, sent once and answered once.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// From the device that made it: the idempotency key.
    pub id: Uuid,
    pub table: String,
    pub row_id: Uuid,
    #[serde(rename = "type")]
    pub kind: EventKind,
    /// The row's version the change started from, 0 for a create.
    pub base_version: i64,
    /// The changed fields' values as the device saw them before the change.
    pub base: Fields,
    /// The values the person chose.
    pub changes: Fields,
    /// The clock of the device that made it, for ordering and display only.
    pub made_at: DateTime<Utc>,
}

impl Event {
    pub fn create(table: &str, row_id: Uuid, changes: Fields, made_at: DateTime<Utc>) -> Event {
        Event {
            id: Uuid::now_v7(),
            table: table.to_string(),
            row_id,
            kind: EventKind::Create,
            base_version: 0,
            base: Fields::new(),
            changes,
            made_at,
        }
    }

    pub fn update(
        table: &str,
        row_id: Uuid,
        base_version: i64,
        base: Fields,
        changes: Fields,
        made_at: DateTime<Utc>,
    ) -> Event {
        Event {
            id: Uuid::now_v7(),
            table: table.to_string(),
            row_id,
            kind: EventKind::Update,
            base_version,
            base,
            changes,
            made_at,
        }
    }
}

/// A field someone else changed first: the person's value, and the
/// server's that stays.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldConflict {
    pub yours: Value,
    pub server: Value,
}

pub type Conflicts = BTreeMap<String, FieldConflict>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnswerStatus {
    Applied,
    Conflict,
    Rejected,
}

/// The server's answer to one event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer {
    pub id: Uuid,
    pub status: AnswerStatus,
    /// The row's version after the event.
    #[serde(default)]
    pub version: Option<i64>,
    #[serde(default)]
    pub conflicts: Conflicts,
    /// Why it was rejected.
    #[serde(default)]
    pub reason: Option<String>,
    /// The row as it now stands on the server, when it has one.
    #[serde(default)]
    pub server: Option<TableRow>,
}

impl Answer {
    /// Applied, or a conflict when some field did not apply.
    pub fn applied(id: Uuid, version: i64, conflicts: Conflicts, server: TableRow) -> Answer {
        let status = if conflicts.is_empty() {
            AnswerStatus::Applied
        } else {
            AnswerStatus::Conflict
        };
        Answer {
            id,
            status,
            version: Some(version),
            conflicts,
            reason: None,
            server: Some(server),
        }
    }

    pub fn rejected(id: Uuid, reason: impl Into<String>, server: Option<TableRow>) -> Answer {
        Answer {
            id,
            status: AnswerStatus::Rejected,
            version: None,
            conflicts: Conflicts::new(),
            reason: Some(reason.into()),
            server,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventStatus {
    /// On a device: not yet answered by the server.
    Pending,
    Applied,
    Conflict,
    Rejected,
}

impl EventStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            EventStatus::Pending => "pending",
            EventStatus::Applied => "applied",
            EventStatus::Conflict => "conflict",
            EventStatus::Rejected => "rejected",
        }
    }

    pub fn parse(text: &str) -> Option<EventStatus> {
        match text {
            "pending" => Some(EventStatus::Pending),
            "applied" => Some(EventStatus::Applied),
            "conflict" => Some(EventStatus::Conflict),
            "rejected" => Some(EventStatus::Rejected),
            _ => None,
        }
    }
}

/// An event as `app_events` keeps it: with its answer on the server, or
/// its place in the queue on a device.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredEvent {
    #[serde(flatten)]
    pub event: Event,
    /// Who made it: on the server the user the token stood for, on a
    /// device the user signed in.
    pub user_id: Uuid,
    pub status: EventStatus,
    #[serde(default)]
    pub conflicts: Conflicts,
    /// On the server, the row's version after the event.
    pub version: Option<i64>,
    /// On the server, why it was rejected; on a device, the reason of a
    /// rejection that kept the event pending.
    pub last_error: Option<String>,
}

impl StoredEvent {
    /// The answer the server recorded, with the row as it stands now.
    pub fn answer(&self, server: Option<TableRow>) -> Answer {
        let status = match self.status {
            EventStatus::Applied => AnswerStatus::Applied,
            EventStatus::Conflict => AnswerStatus::Conflict,
            EventStatus::Rejected | EventStatus::Pending => AnswerStatus::Rejected,
        };
        Answer {
            id: self.event.id,
            status,
            version: self.version,
            conflicts: self.conflicts.clone(),
            reason: self.last_error.clone(),
            server,
        }
    }
}

/// Merges an event's `changes` into a row's `current` values at
/// `version`, field by field. At the server's version every change
/// applies. Behind it, a field the row already holds the device's value
/// for is skipped; a field still at the device's `base` value applies,
/// since nobody else changed it; any other field was changed by someone
/// else first, and the device's value is a conflict.
pub fn merge(
    current: &Fields,
    version: i64,
    base_version: i64,
    base: &Fields,
    changes: &Fields,
) -> (Fields, Conflicts) {
    if base_version == version {
        return (changes.clone(), Conflicts::new());
    }
    let mut applied = Fields::new();
    let mut conflicts = Conflicts::new();
    for (field, value) in changes {
        let server = current.get(field).cloned().unwrap_or(Value::Null);
        if server == *value {
            continue;
        }
        if base.get(field) == Some(&server) {
            applied.insert(field.clone(), value.clone());
        } else {
            conflicts.insert(
                field.clone(),
                FieldConflict {
                    yours: value.clone(),
                    server,
                },
            );
        }
    }
    (applied, conflicts)
}

/// The reason to refuse an event that changes a field it may not, or
/// nothing when every field is allowed.
pub fn not_editable(changes: &Fields, allowed: &[&str]) -> Option<String> {
    let mut refused: Vec<&str> = changes
        .keys()
        .map(String::as_str)
        .filter(|field| !allowed.contains(field))
        .collect();
    if refused.is_empty() {
        return None;
    }
    refused.sort_unstable();
    Some(format!("not_editable: {}", refused.join(", ")))
}

/// Records an event the server answered, in the same transaction as the
/// row, so a retry gets the same answer.
pub async fn record(
    tx: &mut Transaction,
    user_id: Uuid,
    event: &Event,
    answer: &Answer,
) -> Result<(), sqlx::Error> {
    let status = match answer.status {
        AnswerStatus::Applied => EventStatus::Applied,
        AnswerStatus::Conflict => EventStatus::Conflict,
        AnswerStatus::Rejected => EventStatus::Rejected,
    };
    let stored = StoredEvent {
        event: event.clone(),
        user_id,
        status,
        conflicts: answer.conflicts.clone(),
        version: answer.version,
        last_error: answer.reason.clone(),
    };
    sql!(tx, events::insert(&stored))
}

/// Queues an event a device applied, for the next push.
pub async fn queue(tx: &mut Transaction, user_id: Uuid, event: &Event) -> Result<(), sqlx::Error> {
    let stored = StoredEvent {
        event: event.clone(),
        user_id,
        status: EventStatus::Pending,
        conflicts: Conflicts::new(),
        version: None,
        last_error: None,
    };
    sql!(tx, events::insert(&stored))
}

pub async fn get(tx: &mut Transaction, id: Uuid) -> Result<Option<StoredEvent>, sqlx::Error> {
    sql!(tx, events::get(id))
}

/// The events not yet answered, in the order they were made.
pub async fn pending(tx: &mut Transaction) -> Result<Vec<StoredEvent>, sqlx::Error> {
    sql!(tx, events::list_by_status(EventStatus::Pending))
}

/// The conflicts the person has not resolved, newest first.
pub async fn conflicts(tx: &mut Transaction) -> Result<Vec<StoredEvent>, sqlx::Error> {
    let mut conflicts = sql!(tx, events::list_by_status(EventStatus::Conflict))?;
    conflicts.reverse();
    Ok(conflicts)
}

pub async fn count(tx: &mut Transaction, status: EventStatus) -> Result<i64, sqlx::Error> {
    sql!(tx, events::count_by_status(status))
}

/// Whether the row has an event not yet answered, in which case a row
/// from the server must not replace it.
pub async fn has_pending(
    tx: &mut Transaction,
    table: &str,
    row_id: Uuid,
) -> Result<bool, sqlx::Error> {
    sql!(tx, events::has_pending(table, row_id))
}

pub async fn delete(tx: &mut Transaction, id: Uuid) -> Result<(), sqlx::Error> {
    sql!(tx, events::delete(id))
}

pub async fn delete_pending_of_row(
    tx: &mut Transaction,
    table: &str,
    row_id: Uuid,
) -> Result<(), sqlx::Error> {
    sql!(tx, events::delete_pending_of_row(table, row_id))
}

pub async fn mark_conflict(
    tx: &mut Transaction,
    id: Uuid,
    conflicts: &Conflicts,
) -> Result<(), sqlx::Error> {
    sql!(tx, events::mark_conflict(id, conflicts))
}

pub async fn mark_error(tx: &mut Transaction, id: Uuid, reason: &str) -> Result<(), sqlx::Error> {
    sql!(tx, events::mark_error(id, reason))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn fields(value: Value) -> Fields {
        match value {
            Value::Object(fields) => fields,
            _ => panic!("fields are an object"),
        }
    }

    #[test]
    fn events_merge_applies_everything_at_the_servers_version() {
        let (applied, conflicts) = merge(
            &fields(json!({"body": "a"})),
            2,
            2,
            &fields(json!({"body": "a"})),
            &fields(json!({"body": "b"})),
        );
        assert_eq!(applied, fields(json!({"body": "b"})));
        assert!(conflicts.is_empty());
    }

    #[test]
    fn events_merge_keeps_the_change_that_reached_the_server_first() {
        let (applied, conflicts) = merge(
            &fields(json!({"body": "b"})),
            2,
            1,
            &fields(json!({"body": "a"})),
            &fields(json!({"body": "c"})),
        );
        assert!(applied.is_empty());
        assert_eq!(
            conflicts.get("body"),
            Some(&FieldConflict {
                yours: json!("c"),
                server: json!("b")
            })
        );
    }

    #[test]
    fn events_merge_applies_a_field_nobody_else_changed() {
        let (applied, conflicts) = merge(
            &fields(json!({"body": "b", "deleted_at": null})),
            2,
            1,
            &fields(json!({"deleted_at": null})),
            &fields(json!({"deleted_at": "2026-11-08T00:00:00Z"})),
        );
        assert_eq!(
            applied,
            fields(json!({"deleted_at": "2026-11-08T00:00:00Z"}))
        );
        assert!(conflicts.is_empty());
    }

    #[test]
    fn events_merge_skips_a_value_the_server_already_holds() {
        let (applied, conflicts) = merge(
            &fields(json!({"body": "c"})),
            2,
            1,
            &fields(json!({"body": "a"})),
            &fields(json!({"body": "c"})),
        );
        assert!(applied.is_empty());
        assert!(conflicts.is_empty());
    }

    #[test]
    fn events_not_editable_names_the_refused_fields() {
        let changes = fields(json!({"body": "x", "version": 9, "id": "y"}));
        assert_eq!(
            not_editable(&changes, &["body"]),
            Some("not_editable: id, version".to_string())
        );
        assert_eq!(not_editable(&changes, &["body", "id", "version"]), None);
    }
}
