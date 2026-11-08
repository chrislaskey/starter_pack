//! Communities: the soft boundary inside a deployment, see
//! `docs/design/offline-sync.md`. A user joins communities, see
//! `memberships`, and a post is in the communities its owner chose, see
//! `posts_communities`. Any user creates a community and joins it as
//! they do; every user sees every community, so a device can list them
//! and join; nothing changes a community yet, so an update is refused.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

use crate::database::{Transaction, sql};
use crate::deployment::{Deployment, Role, now};
use crate::events::{self, Answer, AnswerStatus, Conflicts, Event, EventKind, Fields};
use crate::memberships::{self, Membership, MembershipError};
use crate::sync::{Cursor, TableRow};
use crate::users::User;

pub const TABLE: &str = "app_communities";
const CREATE_FIELDS: &[&str] = &["name"];
const UPDATE_FIELDS: &[&str] = &[];

/// A community as the pages show it, with the sync columns every synced
/// row carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Community {
    pub id: Uuid,
    pub name: String,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: i64,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_id: Option<Uuid>,
}

/// A community as the index page lists it: with the user's membership
/// when they are in it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityAndMembership {
    pub community: Community,
    pub membership: Option<Membership>,
}

#[derive(Debug, Error)]
pub enum CommunityError {
    #[error("community {id} not found")]
    NotFound { id: Uuid },
    #[error("a community needs a name")]
    BlankName,
    #[error("{0}")]
    Invalid(String),
    #[error("the change was refused: {0}")]
    Rejected(String),
    #[error(transparent)]
    Membership(#[from] MembershipError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<serde_json::Error> for CommunityError {
    fn from(error: serde_json::Error) -> CommunityError {
        CommunityError::Invalid(error.to_string())
    }
}

/// Every community, by name, each with the user's membership when they
/// are in it.
pub async fn list(
    deployment: &Deployment,
    user: &User,
) -> Result<Vec<CommunityAndMembership>, CommunityError> {
    let mut tx = deployment.begin().await?;
    let communities = sql!(&mut tx, communities::list_live())?;
    let memberships = sql!(&mut tx, memberships::list_live_of_user(user.id))?;
    tx.commit().await?;
    Ok(communities
        .into_iter()
        .map(|community| CommunityAndMembership {
            membership: memberships
                .iter()
                .find(|membership| membership.community_id == community.id)
                .cloned(),
            community,
        })
        .collect())
}

/// One community by its id, unless deleted.
pub async fn get(deployment: &Deployment, id: Uuid) -> Result<Community, CommunityError> {
    let mut tx = deployment.begin().await?;
    let community = get_live(&mut tx, id).await?;
    tx.commit().await?;
    Ok(community)
}

/// Makes a community, which the user joins.
pub async fn create(
    deployment: &Deployment,
    user: &User,
    name: &str,
) -> Result<Community, CommunityError> {
    let name = checked_name(name)?;
    let event = Event::create(
        TABLE,
        Uuid::now_v7(),
        fields(json!({ "name": name })),
        now(),
    );
    let mut tx = deployment.begin().await?;
    let community = apply_in(&mut tx, deployment.role, user, &event)
        .await?
        .community()?;
    memberships::join_in(&mut tx, deployment.role, user, community.id).await?;
    tx.commit().await?;
    Ok(community)
}

/// Applies an event of the user's to its community and answers it.
pub async fn apply_event(
    deployment: &Deployment,
    user: &User,
    event: &Event,
) -> Result<Answer, CommunityError> {
    let mut tx = deployment.begin().await?;
    let applied = apply_in(&mut tx, deployment.role, user, event).await?;
    tx.commit().await?;
    Ok(applied.answer)
}

/// The community as it travels on the wire.
pub fn table_row(community: &Community) -> Result<TableRow, CommunityError> {
    Ok(TableRow {
        table: TABLE.to_string(),
        row: serde_json::to_value(community)?,
    })
}

/// Writes the server's row over the device's, unless an event of the
/// device's for it is still pending; true when it was written.
pub(crate) async fn apply_server_row(
    tx: &mut Transaction,
    row: &Value,
) -> Result<bool, CommunityError> {
    let community: Community = serde_json::from_value(row.clone())?;
    if events::has_pending(tx, TABLE, community.id).await? {
        return Ok(false);
    }
    sql!(&mut *tx, communities::upsert(&community))?;
    Ok(true)
}

pub(crate) async fn delete_row(tx: &mut Transaction, id: Uuid) -> Result<(), CommunityError> {
    Ok(sql!(tx, communities::delete_row(id))?)
}

/// A page of the rows the server wrote after the cursor, deleted or not.
pub(crate) async fn list_synced_after(
    tx: &mut Transaction,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<Community>, CommunityError> {
    Ok(sql!(tx, communities::list_synced_after(after, limit))?)
}

pub(crate) async fn list_by_ids(
    tx: &mut Transaction,
    ids: &[Uuid],
) -> Result<Vec<Community>, CommunityError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(sql!(tx, communities::list_by_ids(ids))?)
}

pub(crate) async fn get_in(
    tx: &mut Transaction,
    id: Uuid,
) -> Result<Option<Community>, CommunityError> {
    Ok(sql!(tx, communities::get(id))?)
}

async fn get_live(tx: &mut Transaction, id: Uuid) -> Result<Community, CommunityError> {
    match get_in(tx, id).await? {
        Some(community) if community.deleted_at.is_none() => Ok(community),
        _ => Err(CommunityError::NotFound { id }),
    }
}

struct Applied {
    answer: Answer,
    community: Option<Community>,
}

impl Applied {
    fn community(self) -> Result<Community, CommunityError> {
        match (self.answer.status, self.community) {
            (AnswerStatus::Rejected, _) => Err(CommunityError::Rejected(
                self.answer.reason.unwrap_or_else(|| "rejected".to_string()),
            )),
            (_, Some(community)) => Ok(community),
            (_, None) => Err(CommunityError::NotFound { id: self.answer.id }),
        }
    }
}

/// Applies the event in the transaction: a create writes the row, and
/// an update is refused, since nothing on a community changes yet.
async fn apply_in(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    event: &Event,
) -> Result<Applied, CommunityError> {
    if role == Role::Server
        && let Some(recorded) = events::get(tx, event.id).await?
    {
        let community = get_in(tx, event.row_id).await?;
        let server = community.as_ref().map(table_row).transpose()?;
        return Ok(Applied {
            answer: recorded.answer(server),
            community,
        });
    }

    let allowed = match event.kind {
        EventKind::Create => CREATE_FIELDS,
        EventKind::Update => UPDATE_FIELDS,
    };
    if let Some(reason) = events::not_editable(&event.changes, allowed) {
        let community = get_in(tx, event.row_id).await?;
        return rejected(event, reason, community);
    }
    if event.kind == EventKind::Update {
        let community = get_in(tx, event.row_id).await?;
        return rejected(
            event,
            "not_editable: a community does not change",
            community,
        );
    }
    if let Some(existing) = get_in(tx, event.row_id).await? {
        return rejected(event, "exists", Some(existing));
    }

    let now = now();
    let (version, last_synced_at) = match role {
        Role::Server => (1, Some(now)),
        Role::Device => (0, None),
    };
    let name = event
        .changes
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let name = match checked_name(name) {
        Ok(name) => name,
        Err(error) => return rejected(event, format!("invalid: {error}"), None),
    };
    let community = Community {
        id: event.row_id,
        name,
        inserted_at: now,
        updated_at: now,
        version,
        last_synced_at,
        deleted_at: None,
        deleted_by_id: None,
    };
    sql!(&mut *tx, communities::insert(&community))?;
    let answer = Answer::applied(
        event.id,
        community.version,
        Conflicts::new(),
        table_row(&community)?,
    );
    match role {
        Role::Server => events::record(tx, user.id, event, &answer).await?,
        Role::Device => events::queue(tx, user.id, event).await?,
    }
    Ok(Applied {
        answer,
        community: Some(community),
    })
}

fn rejected(
    event: &Event,
    reason: impl Into<String>,
    community: Option<Community>,
) -> Result<Applied, CommunityError> {
    let server = community.as_ref().map(table_row).transpose()?;
    Ok(Applied {
        answer: Answer::rejected(event.id, reason, server),
        community,
    })
}

fn fields(value: Value) -> Fields {
    match value {
        Value::Object(fields) => fields,
        _ => Fields::new(),
    }
}

/// The name as it is kept: trimmed, and not blank.
fn checked_name(name: &str) -> Result<String, CommunityError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CommunityError::BlankName);
    }
    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{Database, with_each_database};
    use crate::users;

    fn server(database: Database) -> Deployment {
        Deployment::new(database, Role::Server)
    }

    #[tokio::test]
    async fn communities_create_joins_the_maker_and_lists_for_everyone() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = users::create(&deployment, "alice@example.com")
                .await
                .expect("a user");
            let bob = users::create(&deployment, "bob@example.com")
                .await
                .expect("a user");
            let gardening = create(&deployment, &alice, " Gardening ")
                .await
                .expect("a community");
            assert_eq!(gardening.name, "Gardening");
            assert_eq!(gardening.version, 1);
            assert_eq!(
                get(&deployment, gardening.id).await.expect("found"),
                gardening
            );

            let alices = list(&deployment, &alice).await.expect("the list");
            assert_eq!(alices.len(), 1);
            assert_eq!(alices[0].community, gardening);
            let membership = alices[0].membership.as_ref().expect("joined");
            assert_eq!(membership.user_id, alice.id);
            assert_eq!(membership.role, memberships::ROLE_MEMBER);

            let bobs = list(&deployment, &bob).await.expect("the list");
            assert_eq!(bobs[0].community, gardening);
            assert!(bobs[0].membership.is_none());

            let error = create(&deployment, &alice, " ").await.expect_err("blank");
            assert!(matches!(error, CommunityError::BlankName));
            let error = get(&deployment, Uuid::now_v7()).await.expect_err("missing");
            assert!(matches!(error, CommunityError::NotFound { .. }));
        })
        .await;
    }

    #[tokio::test]
    async fn communities_apply_event_refuses_an_update() {
        with_each_database(|database| async move {
            let deployment = server(database);
            let alice = users::create(&deployment, "alice@example.com")
                .await
                .expect("a user");
            let gardening = create(&deployment, &alice, "Gardening")
                .await
                .expect("a community");
            let event = Event::update(
                TABLE,
                gardening.id,
                1,
                fields(json!({ "name": "Gardening" })),
                fields(json!({ "name": "Weeding" })),
                now(),
            );
            let answer = apply_event(&deployment, &alice, &event)
                .await
                .expect("answered");
            assert_eq!(answer.status, AnswerStatus::Rejected);
            assert_eq!(answer.reason.as_deref(), Some("not_editable: name"));
            assert_eq!(
                answer.server.expect("the row").row["name"],
                json!("Gardening")
            );
        })
        .await;
    }
}
