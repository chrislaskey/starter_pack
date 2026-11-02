//! The user's tags and the tags on their posts, written on this device as
//! `App.Data.Tagging` writes them on the server: a tag goes only on its
//! owner's own posts, and a name is unique per owner, ignoring case,
//! among live tags.

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value};

use super::deletes;
use super::store::{change_row, create_row, deleted_now, live_row, own_live_row, row_json};
use crate::changesets;

fn sql_error(error: rusqlite::Error) -> String {
    error.to_string()
}

/// The message `unique_constraint` gives the name on the server.
const NAME_TAKEN: &str = "name has already been taken";

/// Makes a tag, queued as a `create` event. Returns its id.
pub fn create_tag(
    connection: &Connection,
    user_id: &str,
    name: &str,
    now: &str,
) -> Result<String, String> {
    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    let id = make_tag(&transaction, user_id, name, now)?;
    transaction.commit().map_err(sql_error)?;
    Ok(id)
}

fn make_tag(
    connection: &Connection,
    user_id: &str,
    name: &str,
    now: &str,
) -> Result<String, String> {
    let changes = checked_name(user_id, name)?;
    if tag_named(connection, user_id, name)?.is_some() {
        return Err(NAME_TAKEN.into());
    }
    let mut data = Map::new();
    data.insert("owner_id".into(), Value::from(user_id));
    create_row(connection, "app_tags", &data, changes, now)
}

/// Renames the user's own tag, queued as an event.
pub fn rename_tag(
    connection: &Connection,
    user_id: &str,
    tag_id: &str,
    name: &str,
    now: &str,
) -> Result<(), String> {
    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    let tag = own_live_row(&transaction, "app_tags", "owner_id", user_id, tag_id)?;
    let changes = checked_name(user_id, name)?;
    if tag_named(&transaction, user_id, name)?.is_some_and(|id| id != tag_id) {
        return Err(NAME_TAKEN.into());
    }
    change_row(&transaction, user_id, "app_tags", &tag, changes, now)?;
    transaction.commit().map_err(sql_error)
}

/// Deletes the user's own tag, queued as an event, which takes it off
/// their posts and leaves the posts.
pub fn delete_tag(
    connection: &Connection,
    user_id: &str,
    tag_id: &str,
    now: &str,
) -> Result<(), String> {
    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    let tag = own_live_row(&transaction, "app_tags", "owner_id", user_id, tag_id)?;
    change_row(
        &transaction,
        user_id,
        "app_tags",
        &tag,
        deleted_now(now),
        now,
    )?;
    deletes::copy_delete_down(&transaction, "app_tags", tag_id, now, user_id)?;
    transaction.commit().map_err(sql_error)
}

/// Puts the user's tag with the name on their own post, making the tag
/// when they have none by that name, after the post's live tags. A tag
/// taken off before comes back on the same row, as on the server.
pub fn tag_post(
    connection: &Connection,
    user_id: &str,
    post_id: &str,
    name: &str,
    now: &str,
) -> Result<(), String> {
    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    own_live_row(&transaction, "app_posts", "owner_id", user_id, post_id)?;
    let tag_id = match tag_named(&transaction, user_id, name)? {
        Some(id) => id,
        None => make_tag(&transaction, user_id, name, now)?,
    };

    let position: i64 = transaction
        .query_row(
            "SELECT count(*) FROM app_posts_tags WHERE post_id = ?1 AND deleted_at IS NULL",
            [post_id],
            |row| row.get(0),
        )
        .map_err(sql_error)?;
    let pair: Option<String> = transaction
        .query_row(
            "SELECT id FROM app_posts_tags WHERE post_id = ?1 AND tag_id = ?2",
            params![post_id, tag_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(sql_error)?;

    match pair {
        None => {
            let mut changes = Map::new();
            changes.insert("post_id".into(), Value::from(post_id));
            changes.insert("tag_id".into(), Value::from(tag_id.as_str()));
            changes.insert("position".into(), Value::from(position));
            changesets::checked(changesets::post_tag::changeset(&Map::new(), &changes))?;
            create_row(&transaction, "app_posts_tags", &Map::new(), changes, now)?;
        }
        Some(id) => {
            let row = row_json(&transaction, "app_posts_tags", &id)?.ok_or("That tag is gone")?;
            if row.get("deleted_at").is_some_and(|at| !at.is_null()) {
                let mut changes = Map::new();
                changes.insert("deleted_at".into(), Value::Null);
                changes.insert("position".into(), Value::from(position));
                change_row(&transaction, user_id, "app_posts_tags", &row, changes, now)?;
            }
        }
    }
    transaction.commit().map_err(sql_error)
}

/// Takes a tag off the user's own post, queued as an event.
pub fn untag(
    connection: &Connection,
    user_id: &str,
    posts_tags_id: &str,
    now: &str,
) -> Result<(), String> {
    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    let row = live_row(&transaction, "app_posts_tags", posts_tags_id)?
        .ok_or("That tag is off the post already")?;
    let post_id = row.get("post_id").and_then(Value::as_str).unwrap_or("");
    own_live_row(&transaction, "app_posts", "owner_id", user_id, post_id)?;
    change_row(
        &transaction,
        user_id,
        "app_posts_tags",
        &row,
        deleted_now(now),
        now,
    )?;
    transaction.commit().map_err(sql_error)
}

/// The id of the user's live tag with the name, ignoring case and the
/// spaces around it, as `App.Data.Tagging.tag_named/2`.
fn tag_named(connection: &Connection, user_id: &str, name: &str) -> Result<Option<String>, String> {
    connection
        .query_row(
            "SELECT id FROM app_tags
             WHERE owner_id = ?1 AND deleted_at IS NULL AND lower(name) = lower(?2)",
            params![user_id, name.trim()],
            |row| row.get(0),
        )
        .optional()
        .map_err(sql_error)
}

/// The name as `App.Data.Tag.changeset/3` checks it, as a change.
fn checked_name(owner_id: &str, name: &str) -> Result<Map<String, Value>, String> {
    let mut data = Map::new();
    data.insert("owner_id".into(), Value::from(owner_id));
    let mut changes = Map::new();
    changes.insert("name".into(), Value::from(name));
    changesets::checked(changesets::tag::changeset(&data, &changes))?;
    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::pages;
    use crate::sync::schema::open_in_memory;
    use crate::sync::store::tests::{ME, server_rows};
    use crate::sync::store::{apply_rows, pending_events, write_post};
    use serde_json::json;

    const AT: &str = "2026-11-02T12:00:00.000000Z";

    fn device_with_my_post() -> (Connection, String) {
        let connection = open_in_memory().unwrap();
        apply_rows(&connection, ME, &server_rows()).unwrap();
        let post_id = write_post(&connection, ME, "mine", &[], AT).unwrap();
        (connection, post_id)
    }

    fn tag_names(connection: &Connection, post_id: &str) -> Vec<String> {
        pages::feed(connection, ME)
            .unwrap()
            .into_iter()
            .find(|post| post.id == post_id)
            .unwrap()
            .tags
            .into_iter()
            .map(|tag| tag.name)
            .collect()
    }

    #[test]
    fn tagging_a_post_makes_the_tag_once_and_places_it_last() {
        let (connection, post_id) = device_with_my_post();

        tag_post(&connection, ME, &post_id, "Elixir", AT).unwrap();
        tag_post(&connection, ME, &post_id, "rust", AT).unwrap();
        tag_post(&connection, ME, &post_id, " elixir ", AT).unwrap();

        assert_eq!(tag_names(&connection, &post_id), vec!["Elixir", "rust"]);
        let tags = pages::tags_page(&connection, ME).unwrap().tags;
        assert_eq!(tags.len(), 2);
        assert_eq!((tags[0].name.as_str(), tags[0].posts), ("Elixir", 1));

        let events = pending_events(&connection).unwrap();
        let positions: Vec<_> = events
            .iter()
            .filter(|event| event.table == "app_posts_tags")
            .map(|event| event.changes["position"].clone())
            .collect();
        assert_eq!(positions, vec![json!(0), json!(1)]);
    }

    #[test]
    fn untagging_then_tagging_again_revives_the_same_row() {
        let (connection, post_id) = device_with_my_post();
        tag_post(&connection, ME, &post_id, "Elixir", AT).unwrap();
        let row_id = pages::feed(&connection, ME).unwrap()[0].tags[0].id.clone();

        untag(&connection, ME, &row_id, AT).unwrap();
        assert!(tag_names(&connection, &post_id).is_empty());
        tag_post(&connection, ME, &post_id, "elixir", AT).unwrap();

        assert_eq!(pages::feed(&connection, ME).unwrap()[0].tags[0].id, row_id);
        let revive = pending_events(&connection).unwrap().pop().unwrap();
        assert_eq!(revive.kind, "update");
        assert_eq!(revive.changes["deleted_at"], Value::Null);
        assert_eq!(revive.changes["position"], json!(0));
    }

    #[test]
    fn a_name_is_unique_per_owner_ignoring_case() {
        let (connection, _post_id) = device_with_my_post();
        let elixir = create_tag(&connection, ME, "Elixir", AT).unwrap();
        let rust = create_tag(&connection, ME, "Rust", AT).unwrap();

        assert_eq!(
            create_tag(&connection, ME, "ELIXIR", AT).unwrap_err(),
            NAME_TAKEN
        );
        assert_eq!(
            rename_tag(&connection, ME, &rust, "elixir", AT).unwrap_err(),
            NAME_TAKEN
        );
        rename_tag(&connection, ME, &elixir, "elixir", AT).unwrap();
        assert_eq!(
            create_tag(&connection, ME, &"x".repeat(51), AT).unwrap_err(),
            "name should be at most 50 character(s)"
        );
    }

    #[test]
    fn deleting_a_tag_takes_it_off_the_posts_and_leaves_them() {
        let (connection, post_id) = device_with_my_post();
        tag_post(&connection, ME, &post_id, "Elixir", AT).unwrap();
        let tag_id = pages::tags_page(&connection, ME).unwrap().tags[0]
            .id
            .clone();
        let row_id = pages::feed(&connection, ME).unwrap()[0].tags[0].id.clone();

        delete_tag(&connection, ME, &tag_id, AT).unwrap();

        assert!(pages::tags_page(&connection, ME).unwrap().tags.is_empty());
        assert!(tag_names(&connection, &post_id).is_empty());
        assert_eq!(
            row_json(&connection, "app_posts_tags", &row_id)
                .unwrap()
                .unwrap()["deleted_at"],
            json!(AT)
        );
        // The name is free again
        create_tag(&connection, ME, "Elixir", AT).unwrap();
    }

    #[test]
    fn only_the_users_own_posts_and_tags() {
        let (connection, _post_id) = device_with_my_post();
        let theirs = "01a17558-0000-7000-8000-0000000000a1";
        assert!(tag_post(&connection, ME, theirs, "Elixir", AT).is_err());

        let tag_id = create_tag(&connection, ME, "Elixir", AT).unwrap();
        let other = crate::sync::store::tests::OTHER;
        assert!(rename_tag(&connection, other, &tag_id, "Mine", AT).is_err());
        assert!(delete_tag(&connection, other, &tag_id, AT).is_err());
    }
}
