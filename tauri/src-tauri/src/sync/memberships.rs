//! The user's memberships, written on this device as
//! `App.Data.Memberships` writes them on the server. A membership is a
//! server-wins row: the device queues it in the outbox and takes back
//! what the server answers.

use rusqlite::{Connection, OptionalExtension, params};

use super::deletes;
use super::store::{self, deleted_now};

fn sql_error(error: rusqlite::Error) -> String {
    error.to_string()
}

/// The user's live membership of the community, if any.
pub fn membership_id(
    connection: &Connection,
    user_id: &str,
    community_id: &str,
) -> Result<Option<String>, String> {
    connection
        .query_row(
            "SELECT id FROM app_users_communities
             WHERE user_id = ?1 AND community_id = ?2 AND deleted_at IS NULL",
            params![user_id, community_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(sql_error)
}

/// Leaves the community: the membership is deleted and queued, the delete
/// copies onto the user's posts there and their comments, and what the
/// user can no longer see is pruned.
pub fn leave(
    connection: &Connection,
    user_id: &str,
    community_id: &str,
    now: &str,
) -> Result<(), String> {
    let transaction = connection.unchecked_transaction().map_err(sql_error)?;
    let id = membership_id(&transaction, user_id, community_id)?
        .ok_or("You are not a member of that community")?;

    store::update_fields(
        &transaction,
        "app_users_communities",
        &id,
        user_id,
        &deleted_now(now),
        now,
    )?;
    store::queue(&transaction, "app_users_communities", &id, now)?;
    deletes::copy_delete_down(&transaction, "app_users_communities", &id, now, user_id)?;
    store::prune(&transaction, user_id)?;
    transaction.commit().map_err(sql_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::pages;
    use crate::sync::schema::open_in_memory;
    use crate::sync::store::tests::{COMMUNITY, ME, MEMBERSHIP, server_rows};
    use crate::sync::store::{apply_rows, outbox, row_json, write_post};
    use serde_json::json;

    const AT: &str = "2026-11-02T12:00:00.000000Z";

    #[test]
    fn leaving_queues_the_membership_and_hides_the_community() {
        let connection = open_in_memory().unwrap();
        apply_rows(&connection, ME, &server_rows()).unwrap();
        let mine = write_post(&connection, ME, "mine", &[COMMUNITY.to_string()], AT).unwrap();

        leave(&connection, ME, COMMUNITY, AT).unwrap();

        assert!(
            pages::communities_page(&connection, ME)
                .unwrap()
                .communities
                .is_empty()
        );
        assert!(
            pages::community_page(&connection, ME, COMMUNITY)
                .unwrap()
                .is_none()
        );
        let membership = row_json(&connection, "app_users_communities", MEMBERSHIP)
            .unwrap()
            .unwrap();
        assert_eq!(membership["deleted_at"], json!(AT));
        assert!(
            outbox(&connection)
                .unwrap()
                .iter()
                .any(|entry| entry.id == MEMBERSHIP)
        );

        // Someone else's post there is pruned, the user's own stays, out of it
        let posts = pages::feed(&connection, ME).unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].id, mine);
        assert!(posts[0].communities.is_empty());
        assert!(leave(&connection, ME, COMMUNITY, AT).is_err());
    }
}
