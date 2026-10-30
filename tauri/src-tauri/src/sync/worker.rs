//! One sync: push the outbox, pull what changed, backfill a
//! community the user just joined, prune what the user can no longer see.
//! See "Push, in detail" and "Leak 4" in `phoenix/docs/design/offline-sync.md`.

use rusqlite::Connection;
use serde::Serialize;

use super::http::{Page, PushResponse, SyncServer};
use super::store::{self, Applied, WireRow};

#[derive(Clone, Debug, Default, Serialize)]
pub struct SyncReport {
    pub pushed: usize,
    pub rejected: usize,
    pub conflicts: usize,
    pub pulled: usize,
    pub backfilled: usize,
    pub pruned: usize,
    pub pending: i64,
    /// The file was pulled from nothing, see `pull`.
    pub resynced: bool,
    /// Why the push failed as a whole, when it did; the pull still ran.
    pub push_error: Option<String>,
}

/// One sync. A push that fails as a whole, such as the server refusing
/// the request, does not stop the pull: the outbox waits for the next
/// sync and the report says why.
pub async fn sync(
    server: &SyncServer,
    connection: &mut Connection,
    user_id: &str,
) -> Result<SyncReport, String> {
    let mut report = SyncReport::default();

    report.push_error = push(server, connection, user_id, &mut report).await.err();
    pull(server, connection, user_id, &mut report).await?;
    for community_id in store::pending_backfills(connection)? {
        backfill(server, connection, user_id, &community_id, &mut report).await?;
        store::remove_backfill(connection, &community_id)?;
    }
    report.pruned = store::prune(connection, user_id)?;
    report.pending = store::pending_count(connection)?;

    Ok(report)
}

/// Pushes the outbox and the pending events, see "Push" in
/// `phoenix/docs/design/offline-sync-client.md`.
async fn push(
    server: &SyncServer,
    connection: &mut Connection,
    user_id: &str,
    report: &mut SyncReport,
) -> Result<(), String> {
    let started = super::now_iso8601();
    let entries = store::outbox(connection)?;
    let events = store::pending_events(connection)?;
    if entries.is_empty() && events.is_empty() {
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

    let response = server.push(rows, events).await?;
    apply_push_response(connection, user_id, &started, response, report)
}

fn apply_push_response(
    connection: &mut Connection,
    user_id: &str,
    started: &str,
    response: PushResponse,
    report: &mut SyncReport,
) -> Result<(), String> {
    for accepted in response.accepted {
        store::set_synced(
            connection,
            &accepted.table,
            &accepted.id,
            &accepted.last_synced_at,
            accepted.version,
        )?;
        store::dequeue(connection, &accepted.table, &accepted.id, started)?;
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
                // Dequeued first, or the server's row would be skipped as a
                // local edit; an edit made meanwhile is queued again below
                store::dequeue(connection, &rejected.table, &rejected.id, started)?;
                apply_page(connection, user_id, &[server_row])?;
            }
            None => {
                store::record_failure(connection, &rejected.table, &rejected.id, &rejected.reason)?
            }
        }
        report.rejected += 1;
    }

    let mut answered_rows = Vec::new();
    for answer in response.events {
        match answer.status.as_str() {
            "applied" => {
                store::delete_event(connection, &answer.id)?;
                report.pushed += 1;
            }
            "conflict" => {
                store::mark_conflict(connection, &answer.id, &answer.conflicts)?;
                report.pushed += 1;
                report.conflicts += 1;
            }
            _ => {
                let reason = answer.reason.as_deref().unwrap_or("rejected");
                log::info!("Event {} was rejected: {reason}", answer.id);
                if answer.server.is_some() {
                    store::delete_event(connection, &answer.id)?;
                } else {
                    store::record_event_failure(connection, &answer.id, reason)?;
                }
                report.rejected += 1;
            }
        }
        answered_rows.extend(answer.server);
    }
    apply_page(connection, user_id, &answered_rows)?;

    Ok(())
}

/// Pulls every page from the stored cursor and applies it. A membership
/// that arrives for the user records the backfill to do.
async fn pull(
    server: &SyncServer,
    connection: &mut Connection,
    user_id: &str,
    report: &mut SyncReport,
) -> Result<(), String> {
    let mut cursor = store::cursor(connection)?;
    let mut after = None;

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

        report.pulled += apply_page(connection, user_id, &page.rows)?.rows;

        match page.next {
            Some(token) => after = Some(token),
            None => {
                if let Some(cursor) = page.cursor {
                    store::save_cursor(connection, &cursor)?;
                }
                return Ok(());
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
    use crate::sync::store::tests::{ME, server_rows};
    use serde_json::{Value, json};
    use std::env;

    const AT: &str = "2026-10-30T12:00:00.000000Z";

    fn device_with_my_post() -> (Connection, String) {
        let connection = open_in_memory().unwrap();
        store::apply_rows(&connection, ME, &server_rows()).unwrap();
        let mut post = server_rows()[5].clone();
        post.row["author_id"] = json!(ME);
        post.row.insert("version".into(), json!(3));
        store::apply_rows(&connection, ME, &[post.clone()]).unwrap();
        (connection, post.row["id"].as_str().unwrap().to_string())
    }

    fn server_post(connection: &Connection, id: &str, body: &str, version: i64) -> Value {
        let mut row = store::row_json(connection, "app_posts", id)
            .unwrap()
            .unwrap();
        row.insert("body".into(), json!(body));
        row.insert("version".into(), json!(version));
        json!({"table": "app_posts", "row": row})
    }

    fn answer(connection: &mut Connection, response: Value) -> SyncReport {
        let mut report = SyncReport::default();
        let response: PushResponse = serde_json::from_value(response).unwrap();
        apply_push_response(connection, ME, AT, response, &mut report).unwrap();
        report
    }

    fn body(connection: &Connection, id: &str) -> Value {
        store::row_json(connection, "app_posts", id)
            .unwrap()
            .unwrap()["body"]
            .clone()
    }

    #[test]
    fn an_applied_event_is_dropped_and_the_server_row_brings_the_new_version() {
        let (mut connection, post_id) = device_with_my_post();
        store::edit_post(&connection, ME, &post_id, "edited", AT).unwrap();
        let event = store::pending_events(&connection).unwrap().remove(0);
        let server = server_post(&connection, &post_id, "edited", 4);

        let report = answer(
            &mut connection,
            json!({"accepted": [], "rejected": [], "events": [
                {"id": event.id, "status": "applied", "version": 4, "conflicts": {}, "server": server}
            ]}),
        );

        assert_eq!((report.pushed, report.conflicts), (1, 0));
        assert!(store::pending_events(&connection).unwrap().is_empty());
        let row = store::row_json(&connection, "app_posts", &post_id)
            .unwrap()
            .unwrap();
        assert_eq!(row["version"], json!(4));
        assert_eq!(row["body"], json!("edited"));
    }

    #[test]
    fn a_conflict_is_kept_for_the_person_and_the_device_shows_the_servers_value() {
        let (mut connection, post_id) = device_with_my_post();
        store::edit_post(&connection, ME, &post_id, "mine", AT).unwrap();
        let event = store::pending_events(&connection).unwrap().remove(0);
        let server = server_post(&connection, &post_id, "theirs", 4);

        let report = answer(
            &mut connection,
            json!({"accepted": [], "rejected": [], "events": [
                {"id": event.id, "status": "conflict", "version": 4,
                 "conflicts": {"body": {"yours": "mine", "server": "theirs"}}, "server": server}
            ]}),
        );

        assert_eq!(report.conflicts, 1);
        assert_eq!(body(&connection, &post_id), json!("theirs"));
        let conflicts = store::conflicts(&connection).unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].fields[0].yours, json!("mine"));
        assert_eq!(conflicts[0].fields[0].server, json!("theirs"));
        assert_eq!(store::pending_count(&connection).unwrap(), 0);
    }

    #[test]
    fn a_rejected_event_is_dropped_with_the_servers_row_and_kept_without_one() {
        let (mut connection, post_id) = device_with_my_post();
        store::edit_post(&connection, ME, &post_id, "first", AT).unwrap();
        store::edit_post(&connection, ME, &post_id, "second", AT).unwrap();
        let events = store::pending_events(&connection).unwrap();
        let server = server_post(&connection, &post_id, "hello", 3);

        let report = answer(
            &mut connection,
            json!({"accepted": [], "rejected": [], "events": [
                {"id": events[0].id, "status": "rejected", "reason": "not_author", "server": server},
                {"id": events[1].id, "status": "rejected", "reason": "invalid: body", "server": null}
            ]}),
        );

        assert_eq!(report.rejected, 2);
        let pending = store::pending_events(&connection).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, events[1].id);
        assert_eq!(body(&connection, &post_id), json!("second"));
    }

    #[test]
    fn an_event_made_during_the_push_keeps_its_row_from_the_earlier_answer() {
        let (mut connection, post_id) = device_with_my_post();
        store::edit_post(&connection, ME, &post_id, "first", AT).unwrap();
        let pushed = store::pending_events(&connection).unwrap().remove(0);
        store::edit_post(&connection, ME, &post_id, "made while pushing", AT).unwrap();
        let server = server_post(&connection, &post_id, "first", 4);

        answer(
            &mut connection,
            json!({"accepted": [], "rejected": [], "events": [
                {"id": pushed.id, "status": "applied", "version": 4, "conflicts": {}, "server": server}
            ]}),
        );

        assert_eq!(body(&connection, &post_id), json!("made while pushing"));
        assert_eq!(store::pending_events(&connection).unwrap().len(), 1);
    }

    #[test]
    fn an_accepted_row_gets_its_version() {
        let (mut connection, _post_id) = device_with_my_post();
        let community = server_rows()[2].row["id"].as_str().unwrap().to_string();
        store::queue(&connection, "app_communities", &community, AT).unwrap();

        answer(
            &mut connection,
            json!({"accepted": [{"table": "app_communities", "id": community,
                                 "last_synced_at": AT, "version": 2}],
                   "rejected": [], "events": []}),
        );

        let row = store::row_json(&connection, "app_communities", &community)
            .unwrap()
            .unwrap();
        assert_eq!(row["version"], json!(2));
        assert_eq!(row["last_synced_at"], json!(AT));
        assert!(store::outbox(&connection).unwrap().is_empty());
    }

    /// Against a running server, as a device would sync. Needs, see
    /// `docs/offline-sync.md`: `SYNC_SERVER_URL`, `SYNC_TOKEN` and
    /// `SYNC_USER_ID` of a user who is a member of the
    /// community `SYNC_COMMUNITY_ID`.
    #[tokio::test]
    #[ignore = "needs a running Phoenix server with API_SOCKET=true"]
    async fn two_devices_sync_and_resolve_a_conflict_through_a_live_server() {
        let variable = |name: &str| env::var(name).unwrap_or_else(|_| panic!("set {name}"));
        let config = Config {
            server_url: variable("SYNC_SERVER_URL").parse().unwrap(),
        };
        let token = variable("SYNC_TOKEN");
        let user_id = variable("SYNC_USER_ID");
        let community_id = variable("SYNC_COMMUNITY_ID");
        let server = SyncServer::new(&config, &token).unwrap();

        // A first sync on a fresh device pulls everything and backfills the
        // community the membership makes visible
        let mut phone = open_in_memory().unwrap();
        let report = sync(&server, &mut phone, &user_id).await.unwrap();
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
        let report = sync(&server, &mut phone, &user_id).await.unwrap();
        assert_eq!(report.pushed, 1, "the post's create event: {report:?}");
        assert_eq!(
            report.rejected, 1,
            "the community row is server-wins: {report:?}"
        );
        assert_eq!(report.pending, 0);

        let mut laptop = open_in_memory().unwrap();
        sync(&server, &mut laptop, &user_id).await.unwrap();
        let on_laptop = store::feed(&laptop, &user_id).unwrap();
        assert!(
            on_laptop
                .iter()
                .any(|post| post.id == post_id && post.body == "written offline")
        );

        // The edit that reaches the server first stays, whatever the clocks say
        store::edit_post(
            &phone,
            &user_id,
            &post_id,
            "phone",
            &super::super::now_iso8601(),
        )
        .unwrap();
        store::edit_post(
            &laptop,
            &user_id,
            &post_id,
            "laptop, first to sync",
            "2026-01-01T00:00:00.000000Z",
        )
        .unwrap();
        sync(&server, &mut laptop, &user_id).await.unwrap();
        let report = sync(&server, &mut phone, &user_id).await.unwrap();
        assert_eq!(report.conflicts, 1, "{report:?}");

        let body = |connection: &Connection| {
            store::row_json(connection, "app_posts", &post_id)
                .unwrap()
                .unwrap()["body"]
                .clone()
        };
        assert_eq!(body(&phone), "laptop, first to sync");
        let conflict = store::conflicts(&phone).unwrap().remove(0);
        assert_eq!(conflict.fields[0].yours, "phone");

        store::resolve_conflict(
            &phone,
            &user_id,
            &conflict.id,
            true,
            &super::super::now_iso8601(),
        )
        .unwrap();
        let report = sync(&server, &mut phone, &user_id).await.unwrap();
        assert_eq!(report.conflicts, 0, "{report:?}");
        assert!(store::conflicts(&phone).unwrap().is_empty());
        assert_eq!(body(&phone), "phone");
        sync(&server, &mut laptop, &user_id).await.unwrap();
        assert_eq!(body(&laptop), "phone");
    }
}
