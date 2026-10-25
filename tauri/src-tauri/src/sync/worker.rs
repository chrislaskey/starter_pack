//! One sync of one realm: push the outbox, pull what changed, backfill a
//! community the user just joined, prune what the user can no longer see.
//! See "Push, in detail" and "Leak 4" in `phoenix/docs/design/offline-sync.md`.

use rusqlite::Connection;
use serde::Serialize;

use super::http::{Page, SyncServer};
use super::store::{self, Applied, WireRow};

#[derive(Clone, Debug, Default, Serialize)]
pub struct SyncReport {
    pub pushed: usize,
    pub rejected: usize,
    pub pulled: usize,
    pub backfilled: usize,
    pub pruned: usize,
    pub pending: i64,
    /// The realm was pulled from nothing, see `pull`.
    pub resynced: bool,
}

pub async fn sync_realm(
    server: &SyncServer,
    connection: &mut Connection,
    user_id: &str,
) -> Result<SyncReport, String> {
    let mut report = SyncReport::default();

    push(server, connection, user_id, &mut report).await?;
    let joined = pull(server, connection, user_id, &mut report).await?;
    for community_id in joined {
        backfill(server, connection, user_id, &community_id, &mut report).await?;
    }
    report.pruned = store::prune(connection, user_id)?;
    report.pending = store::pending_count(connection)?;

    Ok(report)
}

/// Pushes the outbox. An accepted row gets its `last_synced_at`; a
/// rejected row is replaced by the server's version, or dropped when the
/// server has none. A row edited again while the push was in flight
/// stays queued.
async fn push(
    server: &SyncServer,
    connection: &mut Connection,
    user_id: &str,
    report: &mut SyncReport,
) -> Result<(), String> {
    let started = super::now_iso8601();
    let entries = store::outbox(connection)?;
    if entries.is_empty() {
        return Ok(());
    }

    let mut rows = Vec::new();
    for entry in &entries {
        match store::row_json(connection, &entry.table, &entry.id)? {
            Some(row) => rows.push(WireRow {
                table: entry.table.clone(),
                row,
            }),
            None => store::dequeue(connection, &entry.table, &entry.id, &started)?,
        }
    }

    let response = server.push(rows).await?;

    for accepted in response.accepted {
        store::set_last_synced_at(
            connection,
            &accepted.table,
            &accepted.id,
            &accepted.last_synced_at,
        )?;
        store::dequeue(connection, &accepted.table, &accepted.id, &started)?;
        report.pushed += 1;
    }

    for rejected in response.rejected {
        log::info!(
            "{} {} was rejected: {}",
            rejected.table,
            rejected.id,
            rejected.reason
        );
        match rejected.server {
            Some(server_row) => {
                let same_id = server_row.row.get("id").and_then(serde_json::Value::as_str)
                    == Some(rejected.id.as_str());
                if !same_id {
                    store::delete_row(connection, &rejected.table, &rejected.id)?;
                }
                apply_page(connection, user_id, &[server_row])?;
            }
            None => store::delete_row(connection, &rejected.table, &rejected.id)?,
        }
        store::dequeue(connection, &rejected.table, &rejected.id, &started)?;
        report.rejected += 1;
    }

    Ok(())
}

/// Pulls every page from the stored cursor and applies it. Returns the
/// communities the user just became a member of.
async fn pull(
    server: &SyncServer,
    connection: &mut Connection,
    user_id: &str,
    report: &mut SyncReport,
) -> Result<Vec<String>, String> {
    let mut cursor = store::cursor(connection)?;
    let mut after = None;
    let mut joined = Vec::new();

    loop {
        let page = server.pull(cursor.as_ref(), after.as_ref()).await?;

        if page.resync {
            log::info!("The server purged what this device never saw, pulling from nothing");
            store::reset(connection)?;
            report.resynced = true;
            cursor = None;
            after = None;
            continue;
        }

        let applied = apply_page(connection, user_id, &page.rows)?;
        report.pulled += applied.rows;
        joined.extend(applied.joined_communities);

        match page.next {
            Some(token) => after = Some(token),
            None => {
                if let Some(cursor) = page.cursor {
                    store::save_cursor(connection, &cursor)?;
                }
                return Ok(joined);
            }
        }
    }
}

/// Asks for everything in a community the user just joined, since the
/// cursor cannot see rows that became visible without changing.
async fn backfill(
    server: &SyncServer,
    connection: &mut Connection,
    user_id: &str,
    community_id: &str,
    report: &mut SyncReport,
) -> Result<(), String> {
    let mut after = None;

    loop {
        let page: Page = server.backfill(community_id, after.as_ref()).await?;
        report.backfilled += apply_page(connection, user_id, &page.rows)?.rows;

        match page.next {
            Some(token) => after = Some(token),
            None => return Ok(()),
        }
    }
}

/// Applies a page in one transaction. A page is in cursor order rather
/// than parent-first, so foreign keys are checked at the end.
fn apply_page(
    connection: &mut Connection,
    user_id: &str,
    rows: &[WireRow],
) -> Result<Applied, String> {
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    transaction
        .execute_batch("PRAGMA defer_foreign_keys = ON")
        .map_err(|error| error.to_string())?;
    let applied = store::apply_rows(&transaction, user_id, rows)?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::sync::schema::open_in_memory;
    use std::env;

    /// Against a running server, as a device would sync. Needs, see
    /// `docs/offline-sync.md`: `SYNC_SERVER_URL`, `SYNC_REALM_HOST`,
    /// `SYNC_TOKEN` and `SYNC_USER_ID` of a user who is a member of the
    /// community `SYNC_COMMUNITY_ID`.
    #[tokio::test]
    #[ignore = "needs a running Phoenix server with API_SOCKET=true"]
    async fn two_devices_converge_through_a_live_server() {
        let variable = |name: &str| env::var(name).unwrap_or_else(|_| panic!("set {name}"));
        let config = Config {
            server_url: variable("SYNC_SERVER_URL").parse().unwrap(),
        };
        let host = variable("SYNC_REALM_HOST");
        let token = variable("SYNC_TOKEN");
        let user_id = variable("SYNC_USER_ID");
        let community_id = variable("SYNC_COMMUNITY_ID");
        let server = SyncServer::new(&config, &host, &token).unwrap();

        // A first sync on a fresh device pulls everything and backfills the
        // community the membership makes visible
        let mut phone = open_in_memory().unwrap();
        let report = sync_realm(&server, &mut phone, &user_id).await.unwrap();
        assert!(report.pulled > 0, "{report:?}");
        assert!(store::cursor(&phone).unwrap().is_some());
        assert!(
            store::member_communities(&phone, &user_id)
                .unwrap()
                .iter()
                .any(|community| community.id == community_id)
        );

        // An edit made offline appears on the server after reconnecting
        let now = super::super::now_iso8601();
        let post_id = store::write_post(
            &phone,
            &user_id,
            "written offline",
            std::slice::from_ref(&community_id),
            &now,
        )
        .unwrap();
        let report = sync_realm(&server, &mut phone, &user_id).await.unwrap();
        assert_eq!(report.pushed, 1, "{report:?}");
        assert_eq!(
            report.rejected, 1,
            "the community row is server-wins: {report:?}"
        );
        assert_eq!(report.pending, 0);

        let mut laptop = open_in_memory().unwrap();
        sync_realm(&server, &mut laptop, &user_id).await.unwrap();
        let on_laptop = store::feed(&laptop, &user_id).unwrap();
        assert!(
            on_laptop
                .iter()
                .any(|post| post.id == post_id && post.body == "written offline")
        );

        // The same post edited on both sides while offline converges to the
        // later edit on both devices
        store::edit_post(
            &phone,
            &user_id,
            &post_id,
            "phone, earlier",
            "2026-01-01T00:00:00.000000Z",
        )
        .unwrap();
        store::edit_post(
            &laptop,
            &user_id,
            &post_id,
            "laptop, later",
            &super::super::now_iso8601(),
        )
        .unwrap();
        sync_realm(&server, &mut laptop, &user_id).await.unwrap();
        let report = sync_realm(&server, &mut phone, &user_id).await.unwrap();
        assert_eq!(report.rejected, 1, "{report:?}");

        let body = |connection: &Connection| {
            store::row_json(connection, "posts", &post_id)
                .unwrap()
                .unwrap()["body"]
                .clone()
        };
        assert_eq!(body(&phone), "laptop, later");
        sync_realm(&server, &mut laptop, &user_id).await.unwrap();
        assert_eq!(body(&laptop), "laptop, later");
    }
}
