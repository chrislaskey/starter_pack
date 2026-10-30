//! Reads and writes on the device's file, see `schema`.
//!
//! Server rows are applied as upserts, column for column, exactly as they
//! come off the wire. A local write upserts the row with a client-generated
//! UUIDv7 and the device's time as `updated_at`, and queues it for the
//! next push: in `sync_outbox`, or as an event in `sync_events` for an
//! `EVENT_TABLES` table. `last_synced_at` and `version` are the server's,
//! with placeholders on a new local row: the time, and version 0. See
//! `phoenix/docs/design/offline-sync-client.md`.

use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::changesets;

/// The synced tables, parents first.
pub const TABLES: [&str; 6] = [
    "app_users",
    "app_user_profiles",
    "app_communities",
    "app_users_communities",
    "app_posts",
    "app_posts_communities",
];

pub const EVENT_TABLES: [&str; 2] = ["app_user_profiles", "app_posts"];

/// An event on the wire, see `App.Data.Sync`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WireEvent {
    pub id: String,
    pub table: String,
    pub row_id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub base_version: i64,
    pub base: Map<String, Value>,
    pub changes: Map<String, Value>,
}

/// A change the server kept its own values over.
#[derive(Clone, Debug, Serialize)]
pub struct Conflict {
    pub id: String,
    pub table: String,
    pub row_id: String,
    pub made_at: String,
    pub summary: String,
    pub row_exists: bool,
    pub fields: Vec<ConflictField>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConflictField {
    pub field: String,
    pub yours: Value,
    /// The value on this device now
    pub server: Value,
}

/// A row on the wire: the table and its columns, see `App.Data.Sync`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WireRow {
    pub table: String,
    pub row: Map<String, Value>,
}

/// The pull cursor: `last_synced_at` and `id` of the last row received.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    pub last_synced_at: String,
    pub id: String,
}

/// What applying a page did.
#[derive(Debug, Default)]
pub struct Applied {
    pub rows: usize,
    /// Communities the user just became a live member of, to backfill.
    pub joined_communities: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Community {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct FeedPost {
    pub id: String,
    pub body: String,
    pub owner_email: String,
    pub inserted_at: String,
    pub communities: Vec<String>,
    pub mine: bool,
    /// Queued for the next push.
    pub pending: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct UserProfile {
    pub id: String,
    pub display_name: String,
    pub bio: String,
    /// Queued for the next push.
    pub pending: bool,
}

#[derive(Clone, Debug)]
pub struct OutboxEntry {
    pub table: String,
    pub id: String,
}

fn sql_error(error: rusqlite::Error) -> String {
    error.to_string()
}

// Server rows

/// Upserts the rows, in order.
pub fn apply_rows(
    connection: &Connection,
    user_id: &str,
    rows: &[WireRow],
) -> Result<Applied, String> {
    let mut applied = Applied::default();

    for row in rows {
        if !TABLES.contains(&row.table.as_str()) {
            return Err(format!("unknown table {}", row.table));
        }
        let id = row
            .row
            .get("id")
            .and_then(Value::as_str)
            .ok_or("a row without an id")?;

        // A local edit waiting to be pushed is kept, whatever the server has
        if queued(connection, &row.table, id)? {
            continue;
        }
        // A removed community row of a post this device never held is noise
        if row.table == "app_posts_communities"
            && !is_live(&row.row)
            && !post_exists(connection, &row.row)?
        {
            continue;
        }
        if row.table == "app_users_communities"
            && is_new_live_membership(connection, user_id, &row.row)?
            && let Some(community) = row.row.get("community_id").and_then(Value::as_str)
        {
            add_backfill(connection, community)?;
            applied.joined_communities.push(community.to_string());
        }
        upsert(connection, &row.table, &row.row)?;
        applied.rows += 1;
    }

    Ok(applied)
}

fn is_live(row: &Map<String, Value>) -> bool {
    row.get("deleted_at").is_none_or(Value::is_null)
}

fn post_exists(connection: &Connection, row: &Map<String, Value>) -> Result<bool, String> {
    let Some(post_id) = row.get("post_id").and_then(Value::as_str) else {
        return Ok(false);
    };
    connection
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM app_posts WHERE id = ?1)",
            [post_id],
            |row| row.get(0),
        )
        .map_err(sql_error)
}

// Backfills still to do: a community the user joined, recorded with the
// page that brought the membership and cleared after the last page

pub fn add_backfill(connection: &Connection, community_id: &str) -> Result<(), String> {
    connection
        .execute(
            "INSERT OR IGNORE INTO sync_backfills (community_id) VALUES (?1)",
            [community_id],
        )
        .map_err(sql_error)?;
    Ok(())
}

pub fn pending_backfills(connection: &Connection) -> Result<Vec<String>, String> {
    let mut statement = connection
        .prepare("SELECT community_id FROM sync_backfills ORDER BY community_id")
        .map_err(sql_error)?;
    let ids = statement
        .query_map([], |row| row.get(0))
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(ids)
}

pub fn remove_backfill(connection: &Connection, community_id: &str) -> Result<(), String> {
    connection
        .execute(
            "DELETE FROM sync_backfills WHERE community_id = ?1",
            [community_id],
        )
        .map_err(sql_error)?;
    Ok(())
}

fn is_new_live_membership(
    connection: &Connection,
    user_id: &str,
    row: &Map<String, Value>,
) -> Result<bool, String> {
    let mine = row.get("user_id").and_then(Value::as_str) == Some(user_id);
    let live = row.get("deleted_at").is_none_or(Value::is_null);
    if !mine || !live {
        return Ok(false);
    }

    let id = row
        .get("id")
        .and_then(Value::as_str)
        .ok_or("a row without an id")?;
    let was_live: bool = connection
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM app_users_communities WHERE id = ?1 AND deleted_at IS NULL)",
            [id],
            |row| row.get(0),
        )
        .map_err(sql_error)?;
    Ok(!was_live)
}

fn upsert(connection: &Connection, table: &str, row: &Map<String, Value>) -> Result<(), String> {
    let known = columns(connection, table)?;
    let columns: Vec<&String> = row.keys().filter(|column| known.contains(column)).collect();
    if !columns.iter().any(|column| *column == "id") {
        return Err(format!("a {table} row without an id"));
    }

    let quoted: Vec<String> = columns
        .iter()
        .map(|column| format!("\"{column}\""))
        .collect();
    let placeholders: Vec<String> = (1..=columns.len())
        .map(|index| format!("?{index}"))
        .collect();
    let updates: Vec<String> = quoted
        .iter()
        .filter(|column| *column != "\"id\"")
        .map(|column| format!("{column} = excluded.{column}"))
        .collect();
    let on_conflict = if updates.is_empty() {
        "DO NOTHING".to_string()
    } else {
        format!("DO UPDATE SET {}", updates.join(", "))
    };

    let sql = format!(
        "INSERT INTO \"{table}\" ({}) VALUES ({}) ON CONFLICT(\"id\") {on_conflict}",
        quoted.join(", "),
        placeholders.join(", ")
    );
    let values = columns.iter().map(|column| to_sql(&row[*column]));
    connection
        .execute(&sql, params_from_iter(values))
        .map_err(sql_error)?;
    Ok(())
}

fn columns(connection: &Connection, table: &str) -> Result<Vec<String>, String> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info(\"{table}\")"))
        .map_err(sql_error)?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(names)
}

fn to_sql(value: &Value) -> SqlValue {
    match value {
        Value::Null => SqlValue::Null,
        Value::Bool(flag) => SqlValue::Integer(i64::from(*flag)),
        Value::Number(number) => number
            .as_i64()
            .map(SqlValue::Integer)
            .or_else(|| number.as_f64().map(SqlValue::Real))
            .unwrap_or(SqlValue::Null),
        Value::String(text) => SqlValue::Text(text.clone()),
        other => SqlValue::Text(other.to_string()),
    }
}

/// The row as it would go on the wire, or nothing when it is gone.
pub fn row_json(
    connection: &Connection,
    table: &str,
    id: &str,
) -> Result<Option<Map<String, Value>>, String> {
    if !TABLES.contains(&table) {
        return Err(format!("unknown table {table}"));
    }
    let mut statement = connection
        .prepare(&format!("SELECT * FROM \"{table}\" WHERE id = ?1"))
        .map_err(sql_error)?;
    let names: Vec<String> = statement
        .column_names()
        .iter()
        .map(|name| name.to_string())
        .collect();

    statement
        .query_row([id], |row| {
            let mut map = Map::new();
            for (index, name) in names.iter().enumerate() {
                let value = match row.get_ref(index)? {
                    ValueRef::Null => Value::Null,
                    ValueRef::Integer(integer) => Value::from(integer),
                    ValueRef::Real(real) => Value::from(real),
                    ValueRef::Text(text) => {
                        Value::String(String::from_utf8_lossy(text).into_owned())
                    }
                    ValueRef::Blob(_) => Value::Null,
                };
                map.insert(name.clone(), value);
            }
            Ok(map)
        })
        .optional()
        .map_err(sql_error)
}

pub fn set_synced(
    connection: &Connection,
    table: &str,
    id: &str,
    at: &str,
    version: i64,
) -> Result<(), String> {
    if !TABLES.contains(&table) {
        return Err(format!("unknown table {table}"));
    }
    connection
        .execute(
            &format!("UPDATE \"{table}\" SET last_synced_at = ?1, version = ?2 WHERE id = ?3"),
            params![at, version, id],
        )
        .map_err(sql_error)?;
    Ok(())
}

pub fn delete_row(connection: &Connection, table: &str, id: &str) -> Result<(), String> {
    if !TABLES.contains(&table) {
        return Err(format!("unknown table {table}"));
    }
    connection
        .execute(&format!("DELETE FROM \"{table}\" WHERE id = ?1"), [id])
        .map_err(sql_error)?;
    Ok(())
}

/// Deletes every post not by the user that is no longer in a community
/// the user is a live member of. The post's community rows go with it
/// through the foreign key. Returns how many posts went.
pub fn prune(connection: &Connection, user_id: &str) -> Result<usize, String> {
    connection
        .execute(
            "DELETE FROM app_posts
             WHERE owner_id != ?1
               AND id NOT IN (
                 SELECT pc.post_id FROM app_posts_communities pc
                 JOIN app_users_communities uc
                   ON uc.community_id = pc.community_id AND uc.user_id = ?1 AND uc.deleted_at IS NULL
                 WHERE pc.deleted_at IS NULL
               )",
            [user_id],
        )
        .map_err(sql_error)
}

/// Drops every synced row, the cursor and the outbox, for a resync from
/// nothing. The events stay.
pub fn reset(connection: &Connection) -> Result<(), String> {
    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    for table in TABLES.iter().rev() {
        transaction
            .execute(&format!("DELETE FROM \"{table}\""), [])
            .map_err(sql_error)?;
    }
    transaction
        .execute_batch(
            "UPDATE sync_state SET last_pulled_at = NULL, last_pulled_id = NULL WHERE id = 1;
             DELETE FROM sync_outbox;
             DELETE FROM sync_backfills;",
        )
        .map_err(sql_error)?;
    transaction.commit().map_err(sql_error)
}

// The cursor

pub fn cursor(connection: &Connection) -> Result<Option<Cursor>, String> {
    connection
        .query_row(
            "SELECT last_pulled_at, last_pulled_id FROM sync_state WHERE id = 1",
            [],
            |row| {
                let at: Option<String> = row.get(0)?;
                let id: Option<String> = row.get(1)?;
                Ok(at
                    .zip(id)
                    .map(|(last_synced_at, id)| Cursor { last_synced_at, id }))
            },
        )
        .map_err(sql_error)
}

pub fn save_cursor(connection: &Connection, cursor: &Cursor) -> Result<(), String> {
    connection
        .execute(
            "UPDATE sync_state SET last_pulled_at = ?1, last_pulled_id = ?2 WHERE id = 1",
            params![cursor.last_synced_at, cursor.id],
        )
        .map_err(sql_error)?;
    Ok(())
}

// The outbox

pub fn outbox(connection: &Connection) -> Result<Vec<OutboxEntry>, String> {
    let mut statement = connection
        .prepare(
            "SELECT table_name, row_id FROM sync_outbox ORDER BY queued_at, table_name, row_id",
        )
        .map_err(sql_error)?;
    let entries = statement
        .query_map([], |row| {
            Ok(OutboxEntry {
                table: row.get(0)?,
                id: row.get(1)?,
            })
        })
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(entries)
}

pub fn queue(connection: &Connection, table: &str, id: &str, now: &str) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO sync_outbox (table_name, row_id, queued_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(table_name, row_id) DO UPDATE SET queued_at = excluded.queued_at",
            params![table, id, now],
        )
        .map_err(sql_error)?;
    Ok(())
}

/// Whether the row has an outbox entry or a pending event.
pub fn queued(connection: &Connection, table: &str, id: &str) -> Result<bool, String> {
    connection
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM sync_outbox WHERE table_name = ?1 AND row_id = ?2)
                 OR EXISTS (SELECT 1 FROM sync_events WHERE table_name = ?1 AND row_id = ?2 AND status = 'pending')",
            params![table, id],
            |row| row.get(0),
        )
        .map_err(sql_error)
}

/// Keeps the entry, counting the failed push with the server's reason.
pub fn record_failure(
    connection: &Connection,
    table: &str,
    id: &str,
    error: &str,
) -> Result<(), String> {
    connection
        .execute(
            "UPDATE sync_outbox SET attempts = attempts + 1, last_error = ?3
             WHERE table_name = ?1 AND row_id = ?2",
            params![table, id, error],
        )
        .map_err(sql_error)?;
    Ok(())
}

/// Drops the entry unless the row was edited again after `pushed_at`.
pub fn dequeue(
    connection: &Connection,
    table: &str,
    id: &str,
    pushed_at: &str,
) -> Result<(), String> {
    connection
        .execute(
            "DELETE FROM sync_outbox WHERE table_name = ?1 AND row_id = ?2 AND queued_at <= ?3",
            params![table, id, pushed_at],
        )
        .map_err(sql_error)?;
    Ok(())
}

pub fn pending_count(connection: &Connection) -> Result<i64, String> {
    connection
        .query_row(
            "SELECT (SELECT count(*) FROM sync_outbox)
                  + (SELECT count(*) FROM sync_events WHERE status = 'pending')",
            [],
            |row| row.get(0),
        )
        .map_err(sql_error)
}

// The events

#[allow(clippy::too_many_arguments)]
pub fn queue_event(
    connection: &Connection,
    table: &str,
    row_id: &str,
    kind: &str,
    base_version: i64,
    base: Map<String, Value>,
    changes: Map<String, Value>,
    made_at: &str,
) -> Result<String, String> {
    if !EVENT_TABLES.contains(&table) {
        return Err(format!("{table} changes only as rows"));
    }
    let id = uuid::Uuid::now_v7().to_string();
    connection
        .execute(
            "INSERT INTO sync_events (id, table_name, row_id, type, base_version, base, changes, made_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending')",
            params![
                id,
                table,
                row_id,
                kind,
                base_version,
                Value::Object(base).to_string(),
                Value::Object(changes).to_string(),
                made_at
            ],
        )
        .map_err(sql_error)?;
    Ok(id)
}

/// The pending events, in the order they were made.
pub fn pending_events(connection: &Connection) -> Result<Vec<WireEvent>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, table_name, row_id, type, base_version, base, changes FROM sync_events
             WHERE status = 'pending' ORDER BY made_at, id",
        )
        .map_err(sql_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                WireEvent {
                    id: row.get(0)?,
                    table: row.get(1)?,
                    row_id: row.get(2)?,
                    kind: row.get(3)?,
                    base_version: row.get(4)?,
                    base: Map::new(),
                    changes: Map::new(),
                },
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;

    rows.into_iter()
        .map(|(event, base, changes)| {
            Ok(WireEvent {
                base: json_object(&base)?,
                changes: json_object(&changes)?,
                ..event
            })
        })
        .collect()
}

fn json_object(text: &str) -> Result<Map<String, Value>, String> {
    match serde_json::from_str(text).map_err(|error| error.to_string())? {
        Value::Object(map) => Ok(map),
        other => Err(format!("expected a JSON object, got {other}")),
    }
}

pub fn delete_event(connection: &Connection, id: &str) -> Result<(), String> {
    connection
        .execute("DELETE FROM sync_events WHERE id = ?1", [id])
        .map_err(sql_error)?;
    Ok(())
}

pub fn mark_conflict(
    connection: &Connection,
    id: &str,
    conflicts: &Map<String, Value>,
) -> Result<(), String> {
    connection
        .execute(
            "UPDATE sync_events SET status = 'conflict', conflicts = ?2 WHERE id = ?1",
            params![id, Value::Object(conflicts.clone()).to_string()],
        )
        .map_err(sql_error)?;
    Ok(())
}

pub fn record_event_failure(connection: &Connection, id: &str, error: &str) -> Result<(), String> {
    connection
        .execute(
            "UPDATE sync_events SET attempts = attempts + 1, last_error = ?2 WHERE id = ?1",
            params![id, error],
        )
        .map_err(sql_error)?;
    Ok(())
}

/// Newest first.
pub fn conflicts(connection: &Connection) -> Result<Vec<Conflict>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, table_name, row_id, made_at, coalesce(conflicts, '{}') FROM sync_events
             WHERE status = 'conflict' ORDER BY made_at DESC, id DESC",
        )
        .map_err(sql_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;

    rows.into_iter()
        .map(|(id, table, row_id, made_at, conflicts)| {
            let current = row_json(connection, &table, &row_id)?;
            let fields: Vec<ConflictField> = json_object(&conflicts)?
                .into_iter()
                .map(|(field, conflict)| ConflictField {
                    server: match &current {
                        Some(row) => row.get(&field).cloned().unwrap_or(Value::Null),
                        None => conflict.get("server").cloned().unwrap_or(Value::Null),
                    },
                    yours: conflict.get("yours").cloned().unwrap_or(Value::Null),
                    field,
                })
                .collect();
            let body = current
                .as_ref()
                .and_then(|row| row.get("body"))
                .or_else(|| {
                    fields
                        .iter()
                        .find(|field| field.field == "body")
                        .map(|field| &field.yours)
                })
                .and_then(Value::as_str)
                .unwrap_or("");
            let summary = match table.as_str() {
                "app_user_profiles" => "Your profile".to_string(),
                _ => body.lines().next().unwrap_or("").to_string(),
            };

            Ok(Conflict {
                summary,
                row_exists: current.is_some(),
                id,
                table,
                row_id,
                made_at,
                fields,
            })
        })
        .collect()
}

/// Lets the conflict go, or reapplies it as a new event from the row's
/// current version.
pub fn resolve_conflict(
    connection: &Connection,
    user_id: &str,
    event_id: &str,
    reapply: bool,
    now: &str,
) -> Result<(), String> {
    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    let conflict: Option<(String, String, String)> = transaction
        .query_row(
            "SELECT table_name, row_id, coalesce(conflicts, '{}') FROM sync_events
             WHERE id = ?1 AND status = 'conflict'",
            [event_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(sql_error)?;
    let (table, row_id, conflicts) = conflict.ok_or("That change is already resolved")?;

    if reapply {
        let row = row_json(&transaction, &table, &row_id)?
            .ok_or("It is gone from this device, so the change can only be let go")?;
        let mut base = Map::new();
        let mut changes = Map::new();
        for (field, conflict) in json_object(&conflicts)? {
            base.insert(
                field.clone(),
                row.get(&field).cloned().unwrap_or(Value::Null),
            );
            changes.insert(field, conflict.get("yours").cloned().unwrap_or(Value::Null));
        }
        let version = row.get("version").and_then(Value::as_i64).unwrap_or(0);

        update_fields(&transaction, &table, &row_id, user_id, &changes, now)?;
        queue_event(
            &transaction,
            &table,
            &row_id,
            "update",
            version,
            base,
            changes,
            now,
        )?;
    }

    delete_event(&transaction, event_id)?;
    transaction.commit().map_err(sql_error)
}

fn update_fields(
    connection: &Connection,
    table: &str,
    id: &str,
    user_id: &str,
    changes: &Map<String, Value>,
    now: &str,
) -> Result<(), String> {
    let known = columns(connection, table)?;
    let mut values = changes.clone();
    values.insert("updated_at".into(), Value::from(now));
    if let Some(deleted_at) = changes.get("deleted_at") {
        let by = if deleted_at.is_null() {
            Value::Null
        } else {
            Value::from(user_id)
        };
        values.insert("deleted_by_id".into(), by);
    }

    let fields: Vec<(&String, &Value)> = values
        .iter()
        .filter(|(column, _)| {
            known.contains(column)
                && !["id", "version", "last_synced_at"].contains(&column.as_str())
        })
        .collect();
    let sets: Vec<String> = fields
        .iter()
        .enumerate()
        .map(|(index, (column, _))| format!("\"{column}\" = ?{}", index + 2))
        .collect();
    let sql = format!("UPDATE \"{table}\" SET {} WHERE id = ?1", sets.join(", "));
    let parameters = std::iter::once(SqlValue::Text(id.to_string()))
        .chain(fields.iter().map(|(_, value)| to_sql(value)));
    connection
        .execute(&sql, params_from_iter(parameters))
        .map_err(sql_error)?;
    Ok(())
}

// Local writes

/// Writes a post by the user into the communities, queued for the next
/// push. The user must be a live member of each community.
pub fn write_post(
    connection: &Connection,
    user_id: &str,
    body: &str,
    community_ids: &[String],
    now: &str,
) -> Result<String, String> {
    let body = checked_body(user_id, body)?;
    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    let post_id = uuid::Uuid::now_v7().to_string();
    transaction
        .execute(
            "INSERT INTO app_posts (id, owner_id, body, version, last_synced_at, inserted_at, updated_at)
             VALUES (?1, ?2, ?3, 0, ?4, ?4, ?4)",
            params![post_id, user_id, body, now],
        )
        .map_err(sql_error)?;
    let mut changes = Map::new();
    changes.insert("body".into(), Value::from(body.as_str()));
    queue_event(
        &transaction,
        "app_posts",
        &post_id,
        "create",
        0,
        Map::new(),
        changes,
        now,
    )?;

    for community_id in community_ids {
        let membership: Option<String> = transaction
            .query_row(
                "SELECT id FROM app_users_communities WHERE user_id = ?1 AND community_id = ?2 AND deleted_at IS NULL",
                params![user_id, community_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(sql_error)?;
        let membership = membership.ok_or("You are not a member of one of the communities")?;

        let row_id = uuid::Uuid::now_v7().to_string();
        transaction
            .execute(
                "INSERT INTO app_posts_communities (id, post_id, community_id, users_communities_id, version, last_synced_at, inserted_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5, ?5)",
                params![row_id, post_id, community_id, membership, now],
            )
            .map_err(sql_error)?;
        queue(&transaction, "app_posts_communities", &row_id, now)?;
    }

    transaction.commit().map_err(sql_error)?;
    Ok(post_id)
}

/// Edits the user's own post, queued as an event.
pub fn edit_post(
    connection: &Connection,
    user_id: &str,
    post_id: &str,
    body: &str,
    now: &str,
) -> Result<(), String> {
    let body = checked_body(user_id, body)?;
    let mut changes = Map::new();
    changes.insert("body".into(), Value::from(body));
    change_own_row(
        connection,
        user_id,
        "app_posts",
        "owner_id",
        post_id,
        changes,
        now,
    )
}

/// Deletes the user's own post, queued as an event.
pub fn delete_post(
    connection: &Connection,
    user_id: &str,
    post_id: &str,
    now: &str,
) -> Result<(), String> {
    let mut changes = Map::new();
    changes.insert("deleted_at".into(), Value::from(now));
    change_own_row(
        connection,
        user_id,
        "app_posts",
        "owner_id",
        post_id,
        changes,
        now,
    )
}

/// Edits the user's own profile, queued as an event of the fields that
/// changed.
pub fn edit_profile(
    connection: &Connection,
    user_id: &str,
    display_name: &str,
    bio: &str,
    now: &str,
) -> Result<(), String> {
    let profile = own_profile(connection, user_id)?.ok_or("This device has no profile yet")?;
    let blank_to_null = |text: &str| {
        if text.trim().is_empty() {
            Value::Null
        } else {
            Value::from(text)
        }
    };
    let mut params = Map::new();
    params.insert("display_name".into(), blank_to_null(display_name));
    params.insert("bio".into(), blank_to_null(bio));

    let mut data = Map::new();
    data.insert("user_id".into(), Value::from(user_id));
    changesets::checked(changesets::user_profile::changeset(&data, &params))?;

    let changes: Map<String, Value> = params
        .into_iter()
        .filter(|(field, value)| profile.get(field).unwrap_or(&Value::Null) != value)
        .collect();
    if changes.is_empty() {
        return Ok(());
    }
    let id = profile
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    change_own_row(
        connection,
        user_id,
        "app_user_profiles",
        "user_id",
        &id,
        changes,
        now,
    )
}

fn own_profile(
    connection: &Connection,
    user_id: &str,
) -> Result<Option<Map<String, Value>>, String> {
    let id: Option<String> = connection
        .query_row(
            "SELECT id FROM app_user_profiles WHERE user_id = ?1 AND deleted_at IS NULL",
            [user_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(sql_error)?;
    match id {
        Some(id) => row_json(connection, "app_user_profiles", &id),
        None => Ok(None),
    }
}

/// The user's profile as this device has it.
pub fn profile(connection: &Connection, user_id: &str) -> Result<Option<UserProfile>, String> {
    let Some(row) = own_profile(connection, user_id)? else {
        return Ok(None);
    };
    let text = |field: &str| {
        row.get(field)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let id = text("id");
    Ok(Some(UserProfile {
        pending: queued(connection, "app_user_profiles", &id)?,
        display_name: text("display_name"),
        bio: text("bio"),
        id,
    }))
}

/// Changes the user's own row of an event table and queues the event,
/// with the changed fields' values before the change as its base.
fn change_own_row(
    connection: &Connection,
    user_id: &str,
    table: &str,
    owner_column: &str,
    id: &str,
    changes: Map<String, Value>,
    now: &str,
) -> Result<(), String> {
    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    let row = row_json(&transaction, table, id)?
        .filter(|row| {
            row.get(owner_column).and_then(Value::as_str) == Some(user_id)
                && row.get("deleted_at").is_none_or(Value::is_null)
        })
        .ok_or("Not yours, or it is gone")?;
    let version = row.get("version").and_then(Value::as_i64).unwrap_or(0);
    let base = changes
        .keys()
        .map(|field| {
            (
                field.clone(),
                row.get(field).cloned().unwrap_or(Value::Null),
            )
        })
        .collect();

    update_fields(&transaction, table, id, user_id, &changes, now)?;
    queue_event(
        &transaction,
        table,
        id,
        "update",
        version,
        base,
        changes,
        now,
    )?;
    transaction.commit().map_err(sql_error)
}

/// The server's rules for a post, `App.Data.Post.changeset/3`, run here so
/// a post written offline is not refused on push. The owner is data, as
/// the posting flow sets it on the server; the body is what the user
/// typed, kept as the server keeps it.
fn checked_body(owner_id: &str, body: &str) -> Result<String, String> {
    let mut data = Map::new();
    data.insert("owner_id".into(), Value::from(owner_id));
    let mut params = Map::new();
    params.insert("body".into(), Value::from(body));

    let changeset = changesets::checked(changesets::post::changeset(&data, &params))?;
    Ok(changeset
        .changes()
        .get("body")
        .and_then(Value::as_str)
        .unwrap_or(body)
        .to_string())
}

// Reads for the pages

pub fn member_communities(
    connection: &Connection,
    user_id: &str,
) -> Result<Vec<Community>, String> {
    let mut statement = connection
        .prepare(
            "SELECT c.id, c.name FROM app_communities c
             JOIN app_users_communities uc ON uc.community_id = c.id
             WHERE uc.user_id = ?1 AND uc.deleted_at IS NULL AND c.deleted_at IS NULL
             ORDER BY c.name",
        )
        .map_err(sql_error)?;
    let communities = statement
        .query_map([user_id], |row| {
            Ok(Community {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(communities)
}

/// The posts on this device, newest first: everything the file holds is
/// visible to the user, see `prune`.
pub fn feed(connection: &Connection, user_id: &str) -> Result<Vec<FeedPost>, String> {
    let mut statement = connection
        .prepare(
            "SELECT p.id, p.body, p.owner_id, coalesce(u.email, ''), p.inserted_at,
                    EXISTS (SELECT 1 FROM sync_outbox o WHERE o.table_name = 'app_posts' AND o.row_id = p.id)
                    OR EXISTS (SELECT 1 FROM sync_events e WHERE e.table_name = 'app_posts' AND e.row_id = p.id AND e.status = 'pending')
             FROM app_posts p
             LEFT JOIN app_users u ON u.id = p.owner_id
             WHERE p.deleted_at IS NULL
             ORDER BY p.inserted_at DESC
             LIMIT 200",
        )
        .map_err(sql_error)?;
    let posts = statement
        .query_map([], |row| {
            let owner_id: String = row.get(2)?;
            Ok(FeedPost {
                id: row.get(0)?,
                body: row.get(1)?,
                mine: owner_id == user_id,
                owner_email: row.get(3)?,
                inserted_at: row.get(4)?,
                pending: row.get(5)?,
                communities: Vec::new(),
            })
        })
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;

    let mut names = connection
        .prepare(
            "SELECT c.name FROM app_posts_communities pc
             JOIN app_communities c ON c.id = pc.community_id
             WHERE pc.post_id = ?1 AND pc.deleted_at IS NULL
             ORDER BY c.name",
        )
        .map_err(sql_error)?;
    posts
        .into_iter()
        .map(|mut post| {
            post.communities = names
                .query_map([&post.id], |row| row.get(0))
                .map_err(sql_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(sql_error)?;
            Ok(post)
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::sync::schema::open_in_memory;
    use serde_json::json;

    pub const ME: &str = "01a17558-0000-7000-8000-000000000001";
    pub const OTHER: &str = "01a17558-0000-7000-8000-000000000002";
    pub const COMMUNITY: &str = "01a17558-0000-7000-8000-00000000000c";
    pub const MEMBERSHIP: &str = "01a17558-0000-7000-8000-00000000000d";
    const AT: &str = "2026-10-25T12:00:00.000000Z";

    pub fn wire(table: &str, row: Value) -> WireRow {
        let Value::Object(row) = row else {
            panic!("a row is an object")
        };
        WireRow {
            table: table.into(),
            row,
        }
    }

    fn stamped(mut row: Value, at: &str) -> Value {
        for column in ["inserted_at", "updated_at", "last_synced_at"] {
            row[column] = json!(at);
        }
        for column in ["deleted_at", "deleted_by_id"] {
            if row.get(column).is_none() {
                row[column] = Value::Null;
            }
        }
        row
    }

    /// Me, another user, a community both are in, and a post by the other
    /// user in it, as a pull would deliver them.
    pub fn server_rows() -> Vec<WireRow> {
        vec![
            wire(
                "app_users",
                stamped(
                    json!({"id": ME, "email": "me@example.com", "joined_at": AT}),
                    AT,
                ),
            ),
            wire(
                "app_users",
                stamped(
                    json!({"id": OTHER, "email": "other@example.com", "joined_at": AT}),
                    AT,
                ),
            ),
            wire(
                "app_communities",
                stamped(json!({"id": COMMUNITY, "name": "Gardeners"}), AT),
            ),
            wire(
                "app_users_communities",
                stamped(
                    json!({"id": MEMBERSHIP, "user_id": ME, "community_id": COMMUNITY, "role": "member"}),
                    AT,
                ),
            ),
            wire(
                "app_users_communities",
                stamped(
                    json!({"id": "01a17558-0000-7000-8000-00000000000e", "user_id": OTHER, "community_id": COMMUNITY, "role": "member"}),
                    AT,
                ),
            ),
            wire(
                "app_posts",
                stamped(
                    json!({"id": "01a17558-0000-7000-8000-0000000000a1", "owner_id": OTHER, "body": "hello"}),
                    AT,
                ),
            ),
            wire(
                "app_posts_communities",
                stamped(
                    json!({"id": "01a17558-0000-7000-8000-0000000000b1", "post_id": "01a17558-0000-7000-8000-0000000000a1", "community_id": COMMUNITY, "users_communities_id": "01a17558-0000-7000-8000-00000000000e"}),
                    AT,
                ),
            ),
        ]
    }

    #[test]
    fn applies_rows_as_upserts_and_notices_a_new_membership() {
        let connection = open_in_memory().unwrap();

        let applied = apply_rows(&connection, ME, &server_rows()).unwrap();
        assert_eq!(applied.rows, 7);
        assert_eq!(applied.joined_communities, vec![COMMUNITY.to_string()]);

        let posts = feed(&connection, ME).unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].body, "hello");
        assert_eq!(posts[0].owner_email, "other@example.com");
        assert_eq!(posts[0].communities, vec!["Gardeners".to_string()]);
        assert!(!posts[0].mine && !posts[0].pending);

        // The same rows again change nothing, an edit replaces the column
        let later = "2026-10-25T13:00:00.000000Z";
        let mut edited = server_rows();
        edited[5].row["body"] = json!("hello again");
        edited[5].row["updated_at"] = json!(later);
        let applied = apply_rows(&connection, ME, &edited).unwrap();
        assert!(applied.joined_communities.is_empty());
        assert_eq!(feed(&connection, ME).unwrap()[0].body, "hello again");
        assert_eq!(member_communities(&connection, ME).unwrap().len(), 1);
    }

    #[test]
    fn a_local_post_is_queued_with_its_community_rows() {
        let connection = open_in_memory().unwrap();
        apply_rows(&connection, ME, &server_rows()).unwrap();

        let post_id =
            write_post(&connection, ME, "  mine  ", &[COMMUNITY.to_string()], AT).unwrap();

        let events = pending_events(&connection).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].row_id, post_id);
        assert_eq!(events[0].kind, "create");
        assert_eq!(events[0].base_version, 0);
        assert_eq!(events[0].changes["body"], json!("  mine  "));
        let outbox = outbox(&connection).unwrap();
        assert_eq!(outbox.len(), 1);
        assert_eq!(outbox[0].table, "app_posts_communities");
        assert_eq!(pending_count(&connection).unwrap(), 2);

        // The body is kept as typed, as the server keeps it
        let row = row_json(&connection, "app_posts", &post_id)
            .unwrap()
            .unwrap();
        assert_eq!(row["body"], json!("  mine  "));
        assert_eq!(row["owner_id"], json!(ME));
        assert_eq!(row["updated_at"], json!(AT));
        assert_eq!(row["deleted_at"], Value::Null);
        assert_eq!(row["version"], json!(0));

        let posts = feed(&connection, ME).unwrap();
        let post = posts.iter().find(|post| post.id == post_id).unwrap();
        assert!(post.mine && post.pending);

        assert!(write_post(&connection, ME, "", &[], AT).is_err());
        assert!(
            write_post(
                &connection,
                ME,
                "x",
                &["01a17558-0000-7000-8000-0000000000ff".to_string()],
                AT
            )
            .is_err()
        );
    }

    #[test]
    fn dequeue_keeps_an_entry_edited_after_the_push_started() {
        let connection = open_in_memory().unwrap();
        apply_rows(&connection, ME, &server_rows()).unwrap();

        queue(
            &connection,
            "app_communities",
            COMMUNITY,
            "2026-10-25T12:00:01.000000Z",
        )
        .unwrap();
        dequeue(&connection, "app_communities", COMMUNITY, AT).unwrap();
        assert_eq!(pending_count(&connection).unwrap(), 1);

        dequeue(
            &connection,
            "app_communities",
            COMMUNITY,
            "2026-10-25T12:00:01.000000Z",
        )
        .unwrap();
        assert_eq!(pending_count(&connection).unwrap(), 0);
    }

    #[test]
    fn prune_removes_posts_the_user_can_no_longer_see() {
        let connection = open_in_memory().unwrap();
        apply_rows(&connection, ME, &server_rows()).unwrap();
        let mine = write_post(&connection, ME, "mine", &[], AT).unwrap();
        assert_eq!(prune(&connection, ME).unwrap(), 0);

        // Leaving the community arrives as a deleted membership
        let mut left = server_rows()[3].clone();
        left.row["deleted_at"] = json!(AT);
        left.row["deleted_by_id"] = json!(ME);
        apply_rows(&connection, ME, &[left]).unwrap();

        assert_eq!(prune(&connection, ME).unwrap(), 1);
        let posts = feed(&connection, ME).unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].id, mine);
        let rows: i64 = connection
            .query_row("SELECT count(*) FROM app_posts_communities", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(rows, 0, "the post's community rows went with it");
        assert!(member_communities(&connection, ME).unwrap().is_empty());
    }

    #[test]
    fn the_cursor_is_saved_and_read_back() {
        let connection = open_in_memory().unwrap();
        assert_eq!(cursor(&connection).unwrap(), None);

        let saved = Cursor {
            last_synced_at: AT.into(),
            id: COMMUNITY.into(),
        };
        save_cursor(&connection, &saved).unwrap();
        assert_eq!(cursor(&connection).unwrap(), Some(saved));
    }

    #[test]
    fn a_pulled_row_never_overwrites_a_queued_local_edit() {
        let connection = open_in_memory().unwrap();
        apply_rows(&connection, ME, &server_rows()).unwrap();
        let mine = write_post(&connection, ME, "local edit", &[], AT).unwrap();

        let mut from_server = server_rows()[5].clone();
        from_server.row["id"] = json!(mine);
        from_server.row["owner_id"] = json!(ME);
        from_server.row["body"] = json!("server copy");
        let applied = apply_rows(&connection, ME, &[from_server]).unwrap();

        assert_eq!(applied.rows, 0);
        assert_eq!(
            row_json(&connection, "app_posts", &mine).unwrap().unwrap()["body"],
            json!("local edit")
        );
    }

    #[test]
    fn a_new_membership_records_a_backfill_until_it_is_done() {
        let connection = open_in_memory().unwrap();
        assert!(pending_backfills(&connection).unwrap().is_empty());

        apply_rows(&connection, ME, &server_rows()).unwrap();
        assert_eq!(
            pending_backfills(&connection).unwrap(),
            vec![COMMUNITY.to_string()]
        );

        // The same membership again adds nothing
        apply_rows(&connection, ME, &server_rows()).unwrap();
        assert_eq!(pending_backfills(&connection).unwrap().len(), 1);

        remove_backfill(&connection, COMMUNITY).unwrap();
        assert!(pending_backfills(&connection).unwrap().is_empty());
    }

    #[test]
    fn a_removed_row_of_an_unknown_post_is_skipped_and_prune_follows_a_removal() {
        let connection = open_in_memory().unwrap();
        apply_rows(&connection, ME, &server_rows()).unwrap();

        let mut removed = server_rows()[6].clone();
        removed.row["deleted_at"] = json!(AT);
        removed.row["deleted_by_id"] = json!(OTHER);
        apply_rows(&connection, ME, &[removed.clone()]).unwrap();
        assert_eq!(prune(&connection, ME).unwrap(), 1);
        assert!(feed(&connection, ME).unwrap().is_empty());

        // The post is gone now, so the same tombstone again is noise
        let applied = apply_rows(&connection, ME, &[removed]).unwrap();
        assert_eq!(applied.rows, 0);
        let rows: i64 = connection
            .query_row("SELECT count(*) FROM app_posts_communities", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(rows, 0);
    }

    #[test]
    fn reset_drops_every_row_the_cursor_the_outbox_and_the_backfills_but_keeps_the_events() {
        let connection = open_in_memory().unwrap();
        apply_rows(&connection, ME, &server_rows()).unwrap();
        write_post(&connection, ME, "queued", &[], AT).unwrap();
        save_cursor(
            &connection,
            &Cursor {
                last_synced_at: AT.into(),
                id: COMMUNITY.into(),
            },
        )
        .unwrap();

        reset(&connection).unwrap();

        assert!(feed(&connection, ME).unwrap().is_empty());
        assert!(member_communities(&connection, ME).unwrap().is_empty());
        assert_eq!(cursor(&connection).unwrap(), None);
        assert!(outbox(&connection).unwrap().is_empty());
        assert_eq!(pending_events(&connection).unwrap().len(), 1);
        assert!(pending_backfills(&connection).unwrap().is_empty());
    }

    #[test]
    fn a_failed_push_is_counted_on_the_entry_and_a_long_body_is_refused() {
        let connection = open_in_memory().unwrap();
        apply_rows(&connection, ME, &server_rows()).unwrap();
        let mine = write_post(&connection, ME, "draft", &[], AT).unwrap();

        let event = &pending_events(&connection).unwrap()[0];
        record_event_failure(&connection, &event.id, "invalid: body").unwrap();
        let (attempts, last_error): (i64, String) = connection
            .query_row(
                "SELECT attempts, last_error FROM sync_events WHERE row_id = ?1",
                [&mine],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((attempts, last_error.as_str()), (1, "invalid: body"));
        assert!(queued(&connection, "app_posts", &mine).unwrap());

        let long = "x".repeat(10_001);
        assert_eq!(
            write_post(&connection, ME, &long, &[], AT).unwrap_err(),
            "body should be at most 10000 character(s)"
        );
        assert!(edit_post(&connection, ME, &mine, &long, AT).is_err());
    }

    #[test]
    fn edits_only_the_users_own_post_and_queues_it() {
        let connection = open_in_memory().unwrap();
        apply_rows(&connection, ME, &server_rows()).unwrap();
        let mine = write_post(&connection, ME, "draft", &[], AT).unwrap();
        let later = "2026-10-25T13:00:00.000000Z";

        edit_post(&connection, ME, &mine, "final", later).unwrap();
        let row = row_json(&connection, "app_posts", &mine).unwrap().unwrap();
        assert_eq!(row["body"], json!("final"));
        assert_eq!(row["updated_at"], json!(later));
        assert_eq!(
            row["version"],
            json!(0),
            "the client never changes a version"
        );

        let events = pending_events(&connection).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[1].kind, "update");
        assert_eq!(events[1].base_version, 0);
        assert_eq!(events[1].base["body"], json!("draft"));
        assert_eq!(events[1].changes["body"], json!("final"));

        let theirs = "01a17558-0000-7000-8000-0000000000a1";
        assert!(edit_post(&connection, ME, theirs, "mine now", later).is_err());
        assert!(edit_post(&connection, ME, &mine, "  ", later).is_err());
    }

    /// The fixture post, as mine, at version 3.
    fn my_synced_post(connection: &Connection) -> String {
        let mut post = server_rows()[5].clone();
        post.row["owner_id"] = json!(ME);
        post.row.insert("version".into(), json!(3));
        apply_rows(connection, ME, &server_rows()).unwrap();
        apply_rows(connection, ME, &[post.clone()]).unwrap();
        post.row["id"].as_str().unwrap().to_string()
    }

    fn conflict_on_body(connection: &Connection, post_id: &str, server: &str) -> String {
        edit_post(connection, ME, post_id, "mine", AT).unwrap();
        let event = pending_events(connection).unwrap().pop().unwrap();
        let mut conflicts = Map::new();
        conflicts.insert("body".into(), json!({"yours": "mine", "server": server}));
        mark_conflict(connection, &event.id, &conflicts).unwrap();

        let mut row = row_json(connection, "app_posts", post_id).unwrap().unwrap();
        row.insert("body".into(), json!(server));
        row.insert("version".into(), json!(4));
        apply_rows(
            connection,
            ME,
            &[WireRow {
                table: "app_posts".into(),
                row,
            }],
        )
        .unwrap();
        event.id
    }

    #[test]
    fn edits_and_deletes_queue_events_from_the_version_on_the_device() {
        let connection = open_in_memory().unwrap();
        let post_id = my_synced_post(&connection);

        edit_post(&connection, ME, &post_id, "edited", AT).unwrap();
        delete_post(&connection, ME, &post_id, AT).unwrap();

        let events = pending_events(&connection).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].base_version, 3);
        assert_eq!(events[0].base["body"], json!("hello"));
        assert_eq!(events[1].base["deleted_at"], Value::Null);
        assert_eq!(events[1].changes["deleted_at"], json!(AT));

        let row = row_json(&connection, "app_posts", &post_id)
            .unwrap()
            .unwrap();
        assert_eq!(row["deleted_by_id"], json!(ME));
        assert!(
            feed(&connection, ME)
                .unwrap()
                .iter()
                .all(|post| post.id != post_id)
        );
        assert!(
            delete_post(&connection, ME, &post_id, AT).is_err(),
            "already gone"
        );
    }

    #[test]
    fn a_pulled_row_never_overwrites_a_post_with_a_pending_event_but_does_one_with_a_conflict() {
        let connection = open_in_memory().unwrap();
        let post_id = my_synced_post(&connection);
        edit_post(&connection, ME, &post_id, "local edit", AT).unwrap();

        let mut from_server = server_rows()[5].clone();
        from_server.row["owner_id"] = json!(ME);
        from_server.row["body"] = json!("server copy");
        assert_eq!(
            apply_rows(&connection, ME, &[from_server.clone()])
                .unwrap()
                .rows,
            0
        );

        let event = &pending_events(&connection).unwrap()[0];
        mark_conflict(&connection, &event.id, &Map::new()).unwrap();
        assert_eq!(apply_rows(&connection, ME, &[from_server]).unwrap().rows, 1);
        assert_eq!(pending_count(&connection).unwrap(), 0);
    }

    #[test]
    fn a_conflict_shows_the_value_on_the_device_now_next_to_the_persons() {
        let connection = open_in_memory().unwrap();
        let post_id = my_synced_post(&connection);
        let event_id = conflict_on_body(&connection, &post_id, "from the laptop");

        let conflicts = conflicts(&connection).unwrap();
        assert_eq!(conflicts.len(), 1);
        let conflict = &conflicts[0];
        assert_eq!(conflict.id, event_id);
        assert_eq!(conflict.row_id, post_id);
        assert!(conflict.row_exists);
        assert_eq!(conflict.summary, "from the laptop");
        assert_eq!(conflict.fields.len(), 1);
        assert_eq!(conflict.fields[0].field, "body");
        assert_eq!(conflict.fields[0].yours, json!("mine"));
        assert_eq!(conflict.fields[0].server, json!("from the laptop"));
        assert_eq!(pending_count(&connection).unwrap(), 0);
    }

    #[test]
    fn a_conflict_let_go_sends_nothing() {
        let connection = open_in_memory().unwrap();
        let post_id = my_synced_post(&connection);
        let event_id = conflict_on_body(&connection, &post_id, "from the laptop");

        resolve_conflict(&connection, ME, &event_id, false, AT).unwrap();

        assert!(conflicts(&connection).unwrap().is_empty());
        assert!(pending_events(&connection).unwrap().is_empty());
        let row = row_json(&connection, "app_posts", &post_id)
            .unwrap()
            .unwrap();
        assert_eq!(row["body"], json!("from the laptop"));
        assert!(resolve_conflict(&connection, ME, &event_id, false, AT).is_err());
    }

    #[test]
    fn a_conflict_reapplied_is_a_new_event_from_the_servers_version() {
        let connection = open_in_memory().unwrap();
        let post_id = my_synced_post(&connection);
        let event_id = conflict_on_body(&connection, &post_id, "from the laptop");

        resolve_conflict(&connection, ME, &event_id, true, AT).unwrap();

        assert!(conflicts(&connection).unwrap().is_empty());
        let events = pending_events(&connection).unwrap();
        assert_eq!(events.len(), 1);
        assert_ne!(events[0].id, event_id);
        assert_eq!(events[0].kind, "update");
        assert_eq!(events[0].base_version, 4);
        assert_eq!(events[0].base["body"], json!("from the laptop"));
        assert_eq!(events[0].changes["body"], json!("mine"));
        let row = row_json(&connection, "app_posts", &post_id)
            .unwrap()
            .unwrap();
        assert_eq!(row["body"], json!("mine"));
    }

    #[test]
    fn a_conflict_on_a_post_that_is_gone_can_only_be_let_go() {
        let connection = open_in_memory().unwrap();
        let post_id = my_synced_post(&connection);
        let event_id = conflict_on_body(&connection, &post_id, "from the laptop");
        delete_row(&connection, "app_posts", &post_id).unwrap();

        let conflict = &conflicts(&connection).unwrap()[0];
        assert!(!conflict.row_exists);
        assert_eq!(conflict.summary, "mine");
        assert!(resolve_conflict(&connection, ME, &event_id, true, AT).is_err());
        resolve_conflict(&connection, ME, &event_id, false, AT).unwrap();
        assert!(conflicts(&connection).unwrap().is_empty());
    }

    const PROFILE: &str = "01a17558-0000-7000-8000-0000000000f1";

    fn device_with_my_profile() -> Connection {
        let connection = open_in_memory().unwrap();
        apply_rows(&connection, ME, &server_rows()).unwrap();
        let profile = wire(
            "app_user_profiles",
            stamped(
                json!({"id": PROFILE, "user_id": ME, "display_name": null, "bio": "hi", "version": 2}),
                AT,
            ),
        );
        apply_rows(&connection, ME, &[profile]).unwrap();
        connection
    }

    #[test]
    fn a_profile_edit_queues_only_the_fields_that_changed() {
        let connection = device_with_my_profile();

        edit_profile(&connection, ME, "Ada", "hi", AT).unwrap();

        let events = pending_events(&connection).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].table, "app_user_profiles");
        assert_eq!(events[0].base_version, 2);
        assert_eq!(
            events[0].base,
            json!({"display_name": null}).as_object().unwrap().clone()
        );
        assert_eq!(
            events[0].changes,
            json!({"display_name": "Ada"}).as_object().unwrap().clone()
        );
        let shown = profile(&connection, ME).unwrap().unwrap();
        assert_eq!(
            (shown.display_name.as_str(), shown.bio.as_str()),
            ("Ada", "hi")
        );
        assert!(shown.pending);

        // Nothing changed, nothing queued; a blank field is cleared
        edit_profile(&connection, ME, "Ada", "hi", AT).unwrap();
        assert_eq!(pending_events(&connection).unwrap().len(), 1);
        edit_profile(&connection, ME, "Ada", "  ", AT).unwrap();
        assert_eq!(
            pending_events(&connection).unwrap()[1].changes["bio"],
            Value::Null
        );

        assert_eq!(
            edit_profile(&connection, ME, &"x".repeat(101), "", AT).unwrap_err(),
            "display_name should be at most 100 character(s)"
        );
        assert!(edit_profile(&connection, OTHER, "Eve", "", AT).is_err());
    }

    #[test]
    fn a_profile_conflict_is_labelled_as_the_profile() {
        let connection = device_with_my_profile();
        edit_profile(&connection, ME, "Ada", "hi", AT).unwrap();
        let event = pending_events(&connection).unwrap().remove(0);
        let mut answer = Map::new();
        answer.insert(
            "display_name".into(),
            json!({"yours": "Ada", "server": "Grace"}),
        );
        mark_conflict(&connection, &event.id, &answer).unwrap();

        let conflict = &conflicts(&connection).unwrap()[0];
        assert_eq!(conflict.summary, "Your profile");
        assert_eq!(conflict.fields[0].yours, json!("Ada"));
    }
}
