//! Comments on a post in a community, written on this device as
//! `App.Data.Commenting` writes them on the server: any live member of
//! the community comments, and the commenter or the post's owner deletes
//! a comment.

use rusqlite::Connection;
use serde_json::{Map, Value};

use super::memberships::membership_id;
use super::store::{change_row, create_row, deleted_now, live_row, row_json};
use crate::changesets;

fn sql_error(error: rusqlite::Error) -> String {
    error.to_string()
}

fn text<'a>(row: &'a Map<String, Value>, column: &str) -> &'a str {
    row.get(column).and_then(Value::as_str).unwrap_or("")
}

/// Comments on the post in a community, through its live
/// `app_posts_communities` row, queued as a `create` event. Returns the
/// comment's id.
pub fn comment(
    connection: &Connection,
    user_id: &str,
    posts_communities_id: &str,
    body: &str,
    now: &str,
) -> Result<String, String> {
    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    let thread = live_row(&transaction, "app_posts_communities", posts_communities_id)?
        .ok_or("That post is gone from here")?;
    membership_id(&transaction, user_id, text(&thread, "community_id"))?
        .ok_or("You are not a member of that community")?;

    let mut data = Map::new();
    data.insert("owner_id".into(), Value::from(user_id));
    let mut changes = Map::new();
    changes.insert(
        "posts_communities_id".into(),
        Value::from(posts_communities_id),
    );
    changes.insert("body".into(), Value::from(body));
    changesets::checked(changesets::comment::changeset(&data, &changes))?;

    let id = create_row(&transaction, "app_comments", &data, changes, now)?;
    transaction.commit().map_err(sql_error)?;
    Ok(id)
}

/// Deletes a comment, the user's own or one on their post, queued as an
/// event.
pub fn delete_comment(
    connection: &Connection,
    user_id: &str,
    comment_id: &str,
    now: &str,
) -> Result<(), String> {
    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    let comment =
        live_row(&transaction, "app_comments", comment_id)?.ok_or("That comment is gone")?;
    let post_owner_id = row_json(
        &transaction,
        "app_posts_communities",
        text(&comment, "posts_communities_id"),
    )?
    .and_then(|thread| row_json(&transaction, "app_posts", text(&thread, "post_id")).transpose())
    .transpose()?
    .map(|post| text(&post, "owner_id").to_string());

    let commenter = text(&comment, "owner_id") == user_id;
    let post_owner = post_owner_id.as_deref() == Some(user_id);
    if !commenter && !post_owner {
        return Err("Not yours, or it is gone".into());
    }
    change_row(
        &transaction,
        user_id,
        "app_comments",
        &comment,
        deleted_now(now),
        now,
    )?;
    transaction.commit().map_err(sql_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::pages;
    use crate::sync::schema::open_in_memory;
    use crate::sync::store::tests::{COMMUNITY, ME, OTHER, server_rows, wire};
    use crate::sync::store::{apply_rows, pending_events};
    use serde_json::json;

    const AT: &str = "2026-11-02T12:00:00.000000Z";
    const THEIR_THREAD: &str = "01a17558-0000-7000-8000-0000000000b1";

    fn device() -> Connection {
        let connection = open_in_memory().unwrap();
        apply_rows(&connection, ME, &server_rows()).unwrap();
        connection
    }

    fn their_comment(connection: &Connection, thread: &str) -> String {
        let id = "01a17558-0000-7000-8000-0000000000c1";
        let row = json!({"id": id, "posts_communities_id": thread, "owner_id": OTHER, "body": "theirs",
                         "version": 1, "last_synced_at": AT, "inserted_at": AT, "updated_at": AT,
                         "deleted_at": null, "deleted_by_id": null});
        apply_rows(connection, ME, &[wire("app_comments", row)]).unwrap();
        id.to_string()
    }

    #[test]
    fn a_member_comments_and_deletes_their_own_comment() {
        let connection = device();

        let id = comment(&connection, ME, THEIR_THREAD, "Nice", AT).unwrap();

        let page = pages::community_page(&connection, ME, COMMUNITY)
            .unwrap()
            .unwrap();
        let comments = &page.posts[0].comments;
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].body, "Nice");
        assert!(comments[0].deletable && comments[0].pending);
        let event = pending_events(&connection).unwrap().pop().unwrap();
        assert_eq!(event.kind, "create");
        assert_eq!(event.changes["posts_communities_id"], json!(THEIR_THREAD));

        delete_comment(&connection, ME, &id, AT).unwrap();
        let page = pages::community_page(&connection, ME, COMMUNITY)
            .unwrap()
            .unwrap();
        assert!(page.posts[0].comments.is_empty());
    }

    #[test]
    fn someone_elses_comment_is_deleted_only_on_the_users_post() {
        let connection = device();
        let theirs = their_comment(&connection, THEIR_THREAD);

        let page = pages::community_page(&connection, ME, COMMUNITY)
            .unwrap()
            .unwrap();
        assert!(!page.posts[0].comments[0].deletable);
        assert!(delete_comment(&connection, ME, &theirs, AT).is_err());
    }

    #[test]
    fn a_comment_is_checked_with_the_servers_rules() {
        let connection = device();
        assert_eq!(
            comment(&connection, ME, THEIR_THREAD, " ", AT).unwrap_err(),
            "body can't be blank"
        );
        assert!(
            comment(
                &connection,
                ME,
                "01a17558-0000-7000-8000-0000000000ff",
                "Hi",
                AT
            )
            .is_err()
        );
    }
}
