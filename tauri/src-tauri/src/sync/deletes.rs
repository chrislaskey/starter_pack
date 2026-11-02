//! A delete made on this device copied onto the rows below it, as
//! `App.Data.Deletes` does on the server, so the pages hide them at once.
//! See "Deletes" in `phoenix/docs/design/related-resources.md`.
//!
//! The copies are not queued: the server makes its own when the delete
//! reaches it, and they replace these on the next pull.

use rusqlite::{Connection, params};

/// Each table's children, as `(child table, column pointing at the parent)`.
fn children(table: &str) -> &'static [(&'static str, &'static str)] {
    match table {
        "app_posts" => &[
            ("app_links", "post_id"),
            ("app_posts_tags", "post_id"),
            ("app_posts_communities", "post_id"),
        ],
        "app_tags" => &[("app_posts_tags", "tag_id")],
        "app_users_communities" => &[("app_posts_communities", "users_communities_id")],
        "app_posts_communities" => &[("app_comments", "posts_communities_id")],
        _ => &[],
    }
}

/// Sets `deleted_at` and `deleted_by_id` on every live row below the row,
/// all the way down. Rows already deleted are left alone.
pub fn copy_delete_down(
    connection: &Connection,
    table: &str,
    id: &str,
    deleted_at: &str,
    deleted_by_id: &str,
) -> Result<(), String> {
    for (child, column) in children(table) {
        let ids: Vec<String> = {
            let mut statement = connection
                .prepare(&format!(
                    "SELECT id FROM \"{child}\" WHERE \"{column}\" = ?1 AND deleted_at IS NULL"
                ))
                .map_err(|error| error.to_string())?;
            statement
                .query_map([id], |row| row.get(0))
                .map_err(|error| error.to_string())?
                .collect::<Result<_, _>>()
                .map_err(|error| error.to_string())?
        };

        for child_id in ids {
            connection
                .execute(
                    &format!(
                        "UPDATE \"{child}\" SET deleted_at = ?1, deleted_by_id = ?2 WHERE id = ?3"
                    ),
                    params![deleted_at, deleted_by_id, child_id],
                )
                .map_err(|error| error.to_string())?;
            copy_delete_down(connection, child, &child_id, deleted_at, deleted_by_id)?;
        }
    }
    Ok(())
}
