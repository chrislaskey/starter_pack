//! A post's links and its place in a community, written on this device as
//! `App.Data.Posting` writes them on the server. Writing, editing and
//! deleting a post itself are in `store`.

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value};

use super::deletes;
use super::store::{self, change_row, create_row, deleted_now, live_row, own_live_row};
use crate::changesets;

fn sql_error(error: rusqlite::Error) -> String {
    error.to_string()
}

/// Adds a link to the user's own post, queued as a `create` event, once
/// `App.Data.Link.changeset/3` passes it. Returns the link's id.
pub fn add_link(
    connection: &Connection,
    user_id: &str,
    post_id: &str,
    label: &str,
    url: &str,
    now: &str,
) -> Result<String, String> {
    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    own_live_row(&transaction, "app_posts", "owner_id", user_id, post_id)?;

    let mut changes = Map::new();
    changes.insert("post_id".into(), Value::from(post_id));
    changes.insert("label".into(), Value::from(label));
    changes.insert("url".into(), Value::from(url));
    changesets::checked(changesets::link::changeset(&Map::new(), &changes))?;

    let id = create_row(&transaction, "app_links", &Map::new(), changes, now)?;
    transaction.commit().map_err(sql_error)?;
    Ok(id)
}

/// Removes a link from the user's own post, queued as an event.
pub fn remove_link(
    connection: &Connection,
    user_id: &str,
    link_id: &str,
    now: &str,
) -> Result<(), String> {
    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    let link = live_row(&transaction, "app_links", link_id)?.ok_or("That link is gone")?;
    let post_id = link.get("post_id").and_then(Value::as_str).unwrap_or("");
    own_live_row(&transaction, "app_posts", "owner_id", user_id, post_id)?;

    change_row(
        &transaction,
        user_id,
        "app_links",
        &link,
        deleted_now(now),
        now,
    )?;
    transaction.commit().map_err(sql_error)
}

/// Takes the user's own post out of a community: its `app_posts_communities`
/// row is deleted and queued, server-wins, and the delete copies onto the
/// comments there.
pub fn remove_from_community(
    connection: &Connection,
    user_id: &str,
    post_id: &str,
    community_id: &str,
    now: &str,
) -> Result<(), String> {
    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    own_live_row(&transaction, "app_posts", "owner_id", user_id, post_id)?;
    let row_id: Option<String> = transaction
        .query_row(
            "SELECT id FROM app_posts_communities
             WHERE post_id = ?1 AND community_id = ?2 AND deleted_at IS NULL",
            params![post_id, community_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(sql_error)?;
    let row_id = row_id.ok_or("That post is gone from here")?;

    store::update_fields(
        &transaction,
        "app_posts_communities",
        &row_id,
        user_id,
        &deleted_now(now),
        now,
    )?;
    store::queue(&transaction, "app_posts_communities", &row_id, now)?;
    deletes::copy_delete_down(&transaction, "app_posts_communities", &row_id, now, user_id)?;
    transaction.commit().map_err(sql_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::pages;
    use crate::sync::schema::open_in_memory;
    use crate::sync::store::tests::{COMMUNITY, ME, server_rows};
    use crate::sync::store::{apply_rows, pending_events, write_post};
    use serde_json::json;

    const AT: &str = "2026-11-02T12:00:00.000000Z";

    fn device_with_my_post() -> (Connection, String) {
        let connection = open_in_memory().unwrap();
        apply_rows(&connection, ME, &server_rows()).unwrap();
        let post_id = write_post(&connection, ME, "mine", &[COMMUNITY.to_string()], AT).unwrap();
        (connection, post_id)
    }

    #[test]
    fn a_link_is_added_and_removed_as_events() {
        let (connection, post_id) = device_with_my_post();

        let link_id =
            add_link(&connection, ME, &post_id, "Docs", "https://hexdocs.pm", AT).unwrap();
        let post = pages::feed(&connection, ME)
            .unwrap()
            .into_iter()
            .find(|post| post.id == post_id)
            .unwrap();
        assert_eq!(post.links.len(), 1);
        assert_eq!(post.links[0].label, "Docs");
        assert!(post.links[0].pending);

        remove_link(&connection, ME, &link_id, AT).unwrap();
        let events = pending_events(&connection).unwrap();
        let link_events: Vec<_> = events
            .iter()
            .filter(|event| event.table == "app_links")
            .collect();
        assert_eq!(link_events.len(), 2);
        assert_eq!(link_events[0].kind, "create");
        assert_eq!(
            link_events[0].changes,
            json!({"post_id": post_id, "label": "Docs", "url": "https://hexdocs.pm"})
                .as_object()
                .unwrap()
                .clone()
        );
        assert_eq!(link_events[1].changes["deleted_at"], json!(AT));
        assert!(pages::feed(&connection, ME).unwrap()[0].links.is_empty());
    }

    #[test]
    fn a_link_is_checked_with_the_servers_rules_and_only_on_the_users_post() {
        let (connection, post_id) = device_with_my_post();

        assert_eq!(
            add_link(&connection, ME, &post_id, "Docs", "ftp://x", AT).unwrap_err(),
            "url must be an http:// or https:// URL"
        );
        let theirs = "01a17558-0000-7000-8000-0000000000a1";
        assert!(add_link(&connection, ME, theirs, "Docs", "https://x.io", AT).is_err());
    }

    #[test]
    fn deleting_a_post_copies_onto_its_links_and_community_rows() {
        let (connection, post_id) = device_with_my_post();
        let link_id = add_link(&connection, ME, &post_id, "Docs", "https://x.io", AT).unwrap();

        store::delete_post(&connection, ME, &post_id, AT).unwrap();

        let link = store::row_json(&connection, "app_links", &link_id)
            .unwrap()
            .unwrap();
        assert_eq!(link["deleted_at"], json!(AT));
        assert_eq!(link["deleted_by_id"], json!(ME));
        let live_rows: i64 = connection
            .query_row(
                "SELECT count(*) FROM app_posts_communities WHERE post_id = ?1 AND deleted_at IS NULL",
                [&post_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(live_rows, 0);
        let link_events = pending_events(&connection)
            .unwrap()
            .into_iter()
            .filter(|event| event.table == "app_links")
            .count();
        assert_eq!(link_events, 1, "a copied delete is not pushed");
    }

    #[test]
    fn removing_a_post_from_a_community_queues_its_row_and_hides_it_there() {
        let (connection, post_id) = device_with_my_post();

        remove_from_community(&connection, ME, &post_id, COMMUNITY, AT).unwrap();

        let page = pages::community_page(&connection, ME, COMMUNITY)
            .unwrap()
            .unwrap();
        assert!(page.posts.iter().all(|post| post.id != post_id));
        let outbox = store::outbox(&connection).unwrap();
        assert_eq!(outbox.len(), 1);
        assert_eq!(outbox[0].table, "app_posts_communities");
        assert!(remove_from_community(&connection, ME, &post_id, COMMUNITY, AT).is_err());
    }
}
