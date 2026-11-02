//! What each page shows, read from the device's file only, so the pages
//! work with the network off. Each mirrors a Phoenix LiveView: the posts
//! page `App.Web.PostsLive`, a community's page `App.Web.CommunityLive`,
//! and so on. A row with a change waiting to be pushed is `pending`.

use rusqlite::{Connection, Row};
use serde::Serialize;

use super::store::{self, Community, Conflict, Cursor, UserProfile};

fn sql_error(error: rusqlite::Error) -> String {
    error.to_string()
}

/// Whether the row of `table` aliased `alias` has an outbox entry or a
/// pending event, as an SQL expression.
fn pending_sql(table: &str, alias: &str) -> String {
    format!(
        "(EXISTS (SELECT 1 FROM sync_outbox o WHERE o.table_name = '{table}' AND o.row_id = {alias}.id)
          OR EXISTS (SELECT 1 FROM sync_events e WHERE e.table_name = '{table}' AND e.row_id = {alias}.id AND e.status = 'pending'))"
    )
}

fn collect<T>(
    connection: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
    map: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<T>, String> {
    let mut statement = connection.prepare(sql).map_err(sql_error)?;
    let rows = statement
        .query_map(params, map)
        .map_err(sql_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_error)?;
    Ok(rows)
}

/// The sync state every page shows above its own content.
#[derive(Debug, Serialize)]
pub struct SyncStatus {
    pub pending: i64,
    pub conflicts: Vec<Conflict>,
    pub cursor: Option<Cursor>,
}

pub fn sync_status(connection: &Connection) -> Result<SyncStatus, String> {
    Ok(SyncStatus {
        pending: store::pending_count(connection)?,
        conflicts: store::conflicts(connection)?,
        cursor: store::cursor(connection)?,
    })
}

// The communities page

#[derive(Debug, Serialize)]
pub struct CommunitiesPage {
    pub communities: Vec<Community>,
}

pub fn communities_page(connection: &Connection, user_id: &str) -> Result<CommunitiesPage, String> {
    Ok(CommunitiesPage {
        communities: store::member_communities(connection, user_id)?,
    })
}

// The posts page

#[derive(Clone, Debug, Serialize)]
pub struct FeedPost {
    pub id: String,
    pub body: String,
    pub owner_email: String,
    pub inserted_at: String,
    pub communities: Vec<String>,
    pub links: Vec<FeedLink>,
    pub tags: Vec<FeedTag>,
    pub mine: bool,
    pub pending: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct FeedLink {
    pub id: String,
    pub label: String,
    pub url: String,
    pub pending: bool,
}

/// A tag on a post: `id` is the `app_posts_tags` row's.
#[derive(Clone, Debug, Serialize)]
pub struct FeedTag {
    pub id: String,
    pub name: String,
    pub pending: bool,
}

#[derive(Debug, Serialize)]
pub struct PostsPage {
    pub posts: Vec<FeedPost>,
}

pub fn posts_page(connection: &Connection, user_id: &str) -> Result<PostsPage, String> {
    Ok(PostsPage {
        posts: feed(connection, user_id)?,
    })
}

/// The live posts on this device, newest first, with their communities,
/// links and tags: everything the file holds is visible to the user, see
/// `store::prune`.
pub fn feed(connection: &Connection, user_id: &str) -> Result<Vec<FeedPost>, String> {
    let posts = collect(
        connection,
        &format!(
            "SELECT p.id, p.body, p.owner_id, coalesce(u.email, ''), p.inserted_at, {}
             FROM app_posts p
             LEFT JOIN app_users u ON u.id = p.owner_id
             WHERE p.deleted_at IS NULL
             ORDER BY p.inserted_at DESC, p.id DESC
             LIMIT 200",
            pending_sql("app_posts", "p")
        ),
        [],
        |row| {
            let owner_id: String = row.get(2)?;
            Ok(FeedPost {
                id: row.get(0)?,
                body: row.get(1)?,
                mine: owner_id == user_id,
                owner_email: row.get(3)?,
                inserted_at: row.get(4)?,
                pending: row.get(5)?,
                communities: Vec::new(),
                links: Vec::new(),
                tags: Vec::new(),
            })
        },
    )?;

    posts
        .into_iter()
        .map(|mut post| {
            post.communities = collect(
                connection,
                "SELECT c.name FROM app_posts_communities pc
                 JOIN app_communities c ON c.id = pc.community_id
                 WHERE pc.post_id = ?1 AND pc.deleted_at IS NULL
                 ORDER BY c.name",
                [&post.id],
                |row| row.get(0),
            )?;
            post.links = links(connection, &post.id)?;
            post.tags = tags_of(connection, &post.id)?;
            Ok(post)
        })
        .collect()
}

fn links(connection: &Connection, post_id: &str) -> Result<Vec<FeedLink>, String> {
    collect(
        connection,
        &format!(
            "SELECT l.id, l.label, l.url, {} FROM app_links l
             WHERE l.post_id = ?1 AND l.deleted_at IS NULL
             ORDER BY l.inserted_at, l.id",
            pending_sql("app_links", "l")
        ),
        [post_id],
        |row| {
            Ok(FeedLink {
                id: row.get(0)?,
                label: row.get(1)?,
                url: row.get(2)?,
                pending: row.get(3)?,
            })
        },
    )
}

/// The post's live tags by position, then name, as
/// `App.Data.Tagging.tags_of/1` orders them.
fn tags_of(connection: &Connection, post_id: &str) -> Result<Vec<FeedTag>, String> {
    collect(
        connection,
        &format!(
            "SELECT pt.id, t.name, {} FROM app_posts_tags pt
             JOIN app_tags t ON t.id = pt.tag_id
             WHERE pt.post_id = ?1 AND pt.deleted_at IS NULL AND t.deleted_at IS NULL
             ORDER BY pt.position, t.name",
            pending_sql("app_posts_tags", "pt")
        ),
        [post_id],
        |row| {
            Ok(FeedTag {
                id: row.get(0)?,
                name: row.get(1)?,
                pending: row.get(2)?,
            })
        },
    )
}

// A community's page

#[derive(Debug, Serialize)]
pub struct CommunityPage {
    pub community: Community,
    pub member_count: i64,
    pub posts: Vec<ThreadPost>,
}

/// A post in the community, with its thread of comments here.
/// `posts_communities_id` is the post's row in this community, which the
/// comments hang off.
#[derive(Debug, Serialize)]
pub struct ThreadPost {
    pub id: String,
    pub posts_communities_id: String,
    pub body: String,
    pub owner_email: String,
    pub mine: bool,
    pub pending: bool,
    pub comments: Vec<ThreadComment>,
}

#[derive(Debug, Serialize)]
pub struct ThreadComment {
    pub id: String,
    pub body: String,
    pub owner_email: String,
    /// The user's own comment, or one on their post, as
    /// `App.Data.Commenting.deletable?/3`.
    pub deletable: bool,
    pub pending: bool,
}

/// The community and its feed, or nothing when the user is not a live
/// member of it on this device.
pub fn community_page(
    connection: &Connection,
    user_id: &str,
    community_id: &str,
) -> Result<Option<CommunityPage>, String> {
    let community = store::member_communities(connection, user_id)?
        .into_iter()
        .find(|community| community.id == community_id);
    let Some(community) = community else {
        return Ok(None);
    };

    let member_count = connection
        .query_row(
            "SELECT count(*) FROM app_users_communities WHERE community_id = ?1 AND deleted_at IS NULL",
            [community_id],
            |row| row.get(0),
        )
        .map_err(sql_error)?;

    let posts = collect(
        connection,
        &format!(
            "SELECT p.id, pc.id, p.body, p.owner_id, coalesce(u.email, ''), {}
             FROM app_posts_communities pc
             JOIN app_posts p ON p.id = pc.post_id
             LEFT JOIN app_users u ON u.id = p.owner_id
             WHERE pc.community_id = ?1 AND pc.deleted_at IS NULL AND p.deleted_at IS NULL
             ORDER BY p.inserted_at DESC, p.id DESC
             LIMIT 200",
            pending_sql("app_posts", "p")
        ),
        [community_id],
        |row| {
            let owner_id: String = row.get(3)?;
            Ok(ThreadPost {
                id: row.get(0)?,
                posts_communities_id: row.get(1)?,
                body: row.get(2)?,
                mine: owner_id == user_id,
                owner_email: row.get(4)?,
                pending: row.get(5)?,
                comments: Vec::new(),
            })
        },
    )?;

    let posts = posts
        .into_iter()
        .map(|mut post| {
            post.comments = thread(connection, user_id, &post)?;
            Ok(post)
        })
        .collect::<Result<_, String>>()?;

    Ok(Some(CommunityPage {
        community,
        member_count,
        posts,
    }))
}

/// The live comments on the post here, oldest first.
fn thread(
    connection: &Connection,
    user_id: &str,
    post: &ThreadPost,
) -> Result<Vec<ThreadComment>, String> {
    collect(
        connection,
        &format!(
            "SELECT c.id, c.body, c.owner_id, coalesce(u.email, ''), {}
             FROM app_comments c
             LEFT JOIN app_users u ON u.id = c.owner_id
             WHERE c.posts_communities_id = ?1 AND c.deleted_at IS NULL
             ORDER BY c.inserted_at, c.id",
            pending_sql("app_comments", "c")
        ),
        [&post.posts_communities_id],
        |row| {
            let owner_id: String = row.get(2)?;
            Ok(ThreadComment {
                id: row.get(0)?,
                body: row.get(1)?,
                deletable: owner_id == user_id || post.mine,
                owner_email: row.get(3)?,
                pending: row.get(4)?,
            })
        },
    )
}

// The tags page

#[derive(Debug, Serialize)]
pub struct TagsPage {
    pub tags: Vec<TagRow>,
}

/// One of the user's tags, with how many live posts carry it.
#[derive(Debug, Serialize)]
pub struct TagRow {
    pub id: String,
    pub name: String,
    pub posts: i64,
    pub pending: bool,
}

/// The user's live tags by name, as `App.Data.Tagging.tags_with_counts/1`.
pub fn tags_page(connection: &Connection, user_id: &str) -> Result<TagsPage, String> {
    let tags = collect(
        connection,
        &format!(
            "SELECT t.id, t.name,
                    (SELECT count(*) FROM app_posts_tags pt
                     JOIN app_posts p ON p.id = pt.post_id
                     WHERE pt.tag_id = t.id AND pt.deleted_at IS NULL AND p.deleted_at IS NULL),
                    {}
             FROM app_tags t
             WHERE t.owner_id = ?1 AND t.deleted_at IS NULL
             ORDER BY t.name, t.id",
            pending_sql("app_tags", "t")
        ),
        [user_id],
        |row| {
            Ok(TagRow {
                id: row.get(0)?,
                name: row.get(1)?,
                posts: row.get(2)?,
                pending: row.get(3)?,
            })
        },
    )?;
    Ok(TagsPage { tags })
}

// The profile page

#[derive(Debug, Serialize)]
pub struct ProfilePage {
    pub profile: Option<UserProfile>,
}

pub fn profile_page(connection: &Connection, user_id: &str) -> Result<ProfilePage, String> {
    Ok(ProfilePage {
        profile: store::profile(connection, user_id)?,
    })
}
