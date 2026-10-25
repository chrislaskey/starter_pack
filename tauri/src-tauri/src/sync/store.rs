//! Reads and writes on a realm's file, see `schema`.
//!
//! Server rows are applied as upserts, column for column, exactly as they
//! come off the wire. A local write upserts the row with a client-generated
//! UUIDv7 and the device's time as `updated_at`, and queues it in
//! `sync_outbox` for the next push. `last_synced_at` is the server's and
//! never written here, except as a placeholder on a new local row until
//! the push response brings the real one, since the column is not null.

use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The synced tables, parents first.
pub const TABLES: [&str; 5] = [
    "users",
    "communities",
    "users_communities",
    "posts",
    "posts_communities",
];

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
    pub author_email: String,
    pub inserted_at: String,
    pub communities: Vec<String>,
    pub mine: bool,
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
        if row.table == "users_communities"
            && is_new_live_membership(connection, user_id, &row.row)?
            && let Some(community) = row.row.get("community_id").and_then(Value::as_str)
        {
            applied.joined_communities.push(community.to_string());
        }
        upsert(connection, &row.table, &row.row)?;
        applied.rows += 1;
    }

    Ok(applied)
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
            "SELECT EXISTS (SELECT 1 FROM users_communities WHERE id = ?1 AND deleted_at IS NULL)",
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

pub fn set_last_synced_at(
    connection: &Connection,
    table: &str,
    id: &str,
    at: &str,
) -> Result<(), String> {
    if !TABLES.contains(&table) {
        return Err(format!("unknown table {table}"));
    }
    connection
        .execute(
            &format!("UPDATE \"{table}\" SET last_synced_at = ?1 WHERE id = ?2"),
            params![at, id],
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
            "DELETE FROM posts
             WHERE author_id != ?1
               AND id NOT IN (
                 SELECT pc.post_id FROM posts_communities pc
                 JOIN users_communities uc
                   ON uc.community_id = pc.community_id AND uc.user_id = ?1 AND uc.deleted_at IS NULL
                 WHERE pc.deleted_at IS NULL
               )",
            [user_id],
        )
        .map_err(sql_error)
}

/// Drops every synced row and the cursor, for a resync from nothing. The
/// outbox stays: what is queued is pushed before any pull.
pub fn reset(connection: &Connection) -> Result<(), String> {
    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    for table in TABLES.iter().rev() {
        transaction
            .execute(&format!("DELETE FROM \"{table}\""), [])
            .map_err(sql_error)?;
    }
    transaction
        .execute(
            "UPDATE sync_state SET last_pulled_at = NULL, last_pulled_id = NULL WHERE id = 1",
            [],
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
        .query_row("SELECT count(*) FROM sync_outbox", [], |row| row.get(0))
        .map_err(sql_error)
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
    let body = body.trim();
    if body.is_empty() {
        return Err("The post is empty".into());
    }

    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    let post_id = uuid::Uuid::now_v7().to_string();
    transaction
        .execute(
            "INSERT INTO posts (id, author_id, body, last_synced_at, inserted_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4, ?4)",
            params![post_id, user_id, body, now],
        )
        .map_err(sql_error)?;
    queue(&transaction, "posts", &post_id, now)?;

    for community_id in community_ids {
        let membership: Option<String> = transaction
            .query_row(
                "SELECT id FROM users_communities WHERE user_id = ?1 AND community_id = ?2 AND deleted_at IS NULL",
                params![user_id, community_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(sql_error)?;
        let membership = membership.ok_or("You are not a member of one of the communities")?;

        let row_id = uuid::Uuid::now_v7().to_string();
        transaction
            .execute(
                "INSERT INTO posts_communities (id, post_id, community_id, users_communities_id, last_synced_at, inserted_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?5)",
                params![row_id, post_id, community_id, membership, now],
            )
            .map_err(sql_error)?;
        queue(&transaction, "posts_communities", &row_id, now)?;
    }

    transaction.commit().map_err(sql_error)?;
    Ok(post_id)
}

/// Edits the user's own post, queued for the next push.
pub fn edit_post(
    connection: &Connection,
    user_id: &str,
    post_id: &str,
    body: &str,
    now: &str,
) -> Result<(), String> {
    let body = body.trim();
    if body.is_empty() {
        return Err("The post is empty".into());
    }

    let changed = connection
        .execute(
            "UPDATE posts SET body = ?1, updated_at = ?2
             WHERE id = ?3 AND author_id = ?4 AND deleted_at IS NULL",
            params![body, now, post_id, user_id],
        )
        .map_err(sql_error)?;
    if changed == 0 {
        return Err("Not your post, or it is gone".into());
    }
    queue(connection, "posts", post_id, now)
}

// Reads for the pages

pub fn member_communities(
    connection: &Connection,
    user_id: &str,
) -> Result<Vec<Community>, String> {
    let mut statement = connection
        .prepare(
            "SELECT c.id, c.name FROM communities c
             JOIN users_communities uc ON uc.community_id = c.id
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
            "SELECT p.id, p.body, p.author_id, coalesce(u.email, ''), p.inserted_at,
                    EXISTS (SELECT 1 FROM sync_outbox o WHERE o.table_name = 'posts' AND o.row_id = p.id)
             FROM posts p
             LEFT JOIN users u ON u.id = p.author_id
             WHERE p.deleted_at IS NULL
             ORDER BY p.inserted_at DESC
             LIMIT 200",
        )
        .map_err(sql_error)?;
    let posts = statement
        .query_map([], |row| {
            let author_id: String = row.get(2)?;
            Ok(FeedPost {
                id: row.get(0)?,
                body: row.get(1)?,
                mine: author_id == user_id,
                author_email: row.get(3)?,
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
            "SELECT c.name FROM posts_communities pc
             JOIN communities c ON c.id = pc.community_id
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
    pub fn realm_rows() -> Vec<WireRow> {
        vec![
            wire(
                "users",
                stamped(
                    json!({"id": ME, "email": "me@example.com", "joined_at": AT}),
                    AT,
                ),
            ),
            wire(
                "users",
                stamped(
                    json!({"id": OTHER, "email": "other@example.com", "joined_at": AT}),
                    AT,
                ),
            ),
            wire(
                "communities",
                stamped(json!({"id": COMMUNITY, "name": "Gardeners"}), AT),
            ),
            wire(
                "users_communities",
                stamped(
                    json!({"id": MEMBERSHIP, "user_id": ME, "community_id": COMMUNITY, "role": "member"}),
                    AT,
                ),
            ),
            wire(
                "users_communities",
                stamped(
                    json!({"id": "01a17558-0000-7000-8000-00000000000e", "user_id": OTHER, "community_id": COMMUNITY, "role": "member"}),
                    AT,
                ),
            ),
            wire(
                "posts",
                stamped(
                    json!({"id": "01a17558-0000-7000-8000-0000000000a1", "author_id": OTHER, "body": "hello"}),
                    AT,
                ),
            ),
            wire(
                "posts_communities",
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

        let applied = apply_rows(&connection, ME, &realm_rows()).unwrap();
        assert_eq!(applied.rows, 7);
        assert_eq!(applied.joined_communities, vec![COMMUNITY.to_string()]);

        let posts = feed(&connection, ME).unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].body, "hello");
        assert_eq!(posts[0].author_email, "other@example.com");
        assert_eq!(posts[0].communities, vec!["Gardeners".to_string()]);
        assert!(!posts[0].mine && !posts[0].pending);

        // The same rows again change nothing, an edit replaces the column
        let later = "2026-10-25T13:00:00.000000Z";
        let mut edited = realm_rows();
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
        apply_rows(&connection, ME, &realm_rows()).unwrap();

        let post_id =
            write_post(&connection, ME, "  mine  ", &[COMMUNITY.to_string()], AT).unwrap();

        let outbox = outbox(&connection).unwrap();
        assert_eq!(outbox.len(), 2);
        assert_eq!(outbox[0].table, "posts");
        assert_eq!(outbox[0].id, post_id);
        assert_eq!(outbox[1].table, "posts_communities");
        assert_eq!(pending_count(&connection).unwrap(), 2);

        let row = row_json(&connection, "posts", &post_id).unwrap().unwrap();
        assert_eq!(row["body"], json!("mine"));
        assert_eq!(row["author_id"], json!(ME));
        assert_eq!(row["updated_at"], json!(AT));
        assert_eq!(row["deleted_at"], Value::Null);

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
        apply_rows(&connection, ME, &realm_rows()).unwrap();
        let post_id = write_post(&connection, ME, "draft", &[], AT).unwrap();

        queue(
            &connection,
            "posts",
            &post_id,
            "2026-10-25T12:00:01.000000Z",
        )
        .unwrap();
        dequeue(&connection, "posts", &post_id, AT).unwrap();
        assert_eq!(pending_count(&connection).unwrap(), 1);

        dequeue(
            &connection,
            "posts",
            &post_id,
            "2026-10-25T12:00:01.000000Z",
        )
        .unwrap();
        assert_eq!(pending_count(&connection).unwrap(), 0);
    }

    #[test]
    fn prune_removes_posts_the_user_can_no_longer_see() {
        let connection = open_in_memory().unwrap();
        apply_rows(&connection, ME, &realm_rows()).unwrap();
        let mine = write_post(&connection, ME, "mine", &[], AT).unwrap();
        assert_eq!(prune(&connection, ME).unwrap(), 0);

        // Leaving the community arrives as a deleted membership
        let mut left = realm_rows()[3].clone();
        left.row["deleted_at"] = json!(AT);
        left.row["deleted_by_id"] = json!(ME);
        apply_rows(&connection, ME, &[left]).unwrap();

        assert_eq!(prune(&connection, ME).unwrap(), 1);
        let posts = feed(&connection, ME).unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].id, mine);
        let rows: i64 = connection
            .query_row("SELECT count(*) FROM posts_communities", [], |row| {
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
    fn edits_only_the_users_own_post_and_queues_it() {
        let connection = open_in_memory().unwrap();
        apply_rows(&connection, ME, &realm_rows()).unwrap();
        let mine = write_post(&connection, ME, "draft", &[], AT).unwrap();
        let later = "2026-10-25T13:00:00.000000Z";

        edit_post(&connection, ME, &mine, "final", later).unwrap();
        let row = row_json(&connection, "posts", &mine).unwrap().unwrap();
        assert_eq!(row["body"], json!("final"));
        assert_eq!(row["updated_at"], json!(later));
        assert_eq!(pending_count(&connection).unwrap(), 1);

        let theirs = "01a17558-0000-7000-8000-0000000000a1";
        assert!(edit_post(&connection, ME, theirs, "mine now", later).is_err());
        assert!(edit_post(&connection, ME, &mine, "  ", later).is_err());
    }
}
