//! Files: a file a person added, as a row that syncs like a post and
//! bytes kept apart from it, see `docs/design/offline-files-decisions.md`.
//! The row holds the name, the content type and the size; the bytes are
//! in the server's bucket at `files/<owner id>/<file id>` and, on a
//! device, in the account's folder under the file's id, see
//! `deployment::FileStore`. The bytes never change: an update changes
//! the name only. Only its owner sees a file.
//!
//! Every write is an event applied by `apply_event`, as a post's is.
//! What differs is the create: the server writes the row only once the
//! file is in its bucket, and answers `file_not_uploaded` until then.
//! On the web the page asks `create_upload` where to PUT the bytes,
//! sends them, then creates the row. A device keeps the bytes itself,
//! `create_with_bytes`, and the file waits there, its create pending,
//! until a sync has sent them.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use garde::Validate;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use uuid::Uuid;

use crate::database::{Transaction, sql};
use crate::deployment::{Deployment, FileStore, Role, now};
use crate::events::{
    self, Answer, AnswerStatus, Conflicts, Event, EventKind, EventStatus, Fields, StoredEvent,
};
use crate::field_errors::FieldErrors;
#[cfg(feature = "object-storage")]
use crate::storage::{Storage, StorageError};
use crate::sync::{Cursor, SyncError, TableRow, Transport};
use crate::users::User;

pub const TABLE: &str = "app_files";
/// The largest file, 25 MB.
pub const MAX_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_NAME_LENGTH: usize = 255;
/// What a file with no content type of its own is sent as.
pub const UNKNOWN_CONTENT_TYPE: &str = "application/octet-stream";
/// Why the server leaves a create pending: the bytes are not in its
/// bucket yet, so the device sends them and pushes again.
pub const FILE_NOT_UPLOADED: &str = "file_not_uploaded";
/// The fields an event may set when it creates a file.
const CREATE_FIELDS: &[&str] = &["name", "content_type", "size"];
/// The fields an event may change on a file.
const UPDATE_FIELDS: &[&str] = &["name", "deleted_at"];
/// Where files go in the bucket: `files/<owner id>/<file id>`.
const KEY_PREFIX: &str = "files";
/// How long a signed upload URL works.
#[cfg(feature = "object-storage")]
const UPLOAD_URL_LIFETIME: std::time::Duration = std::time::Duration::from_secs(10 * 60);
/// How long a signed download URL works.
#[cfg(feature = "object-storage")]
pub const DOWNLOAD_URL_LIFETIME: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// A file's row, with the sync columns every synced row carries, see
/// `docs/design/offline-sync.md`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct File {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub content_type: String,
    /// In bytes.
    pub size: u64,
    pub inserted_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// The server's version, 0 on a device's file not yet pushed.
    pub version: i64,
    /// When the server last wrote it; none on a device's file not yet pushed.
    pub last_synced_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_id: Option<Uuid>,
}

/// Where a file stands between a device and the server.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileState {
    /// The server has the row and the bytes.
    Synced,
    /// Added on this device, not yet on the server; every sync tries.
    Waiting,
    /// The server will not take it; it stays on this device until the
    /// person tries again or lets it go.
    Refused,
}

/// How a page shows a file's bytes, by its content type. Only these
/// are rendered: an HTML file, say, is a program, and is never loaded
/// as a document. An application adds the types it needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShownAs {
    /// In an `<img>`, animated or not.
    Image,
    /// In a frame, as plain text.
    Text,
    /// In a frame, by the webview's PDF viewer where it has one.
    Pdf,
    /// By its name and size only.
    NotShown,
}

impl ShownAs {
    pub fn of(content_type: &str) -> ShownAs {
        let essence = content_type.split(';').next().unwrap_or_default().trim();
        match essence {
            "text/plain" => ShownAs::Text,
            "application/pdf" => ShownAs::Pdf,
            image if image.starts_with("image/") => ShownAs::Image,
            _ => ShownAs::NotShown,
        }
    }
}

/// A file as the pages show it: with how its bytes are shown, its
/// state, and why the last try to send it failed or why it was refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileWithState {
    pub file: File,
    pub shown_as: ShownAs,
    pub state: FileState,
    pub error: Option<String>,
}

/// A file to add: what the person picked, under the id its bytes are
/// already kept under.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Validate)]
pub struct FileNew {
    #[garde(skip)]
    pub id: Uuid,
    #[garde(custom(is_file_name))]
    pub name: String,
    #[garde(custom(is_content_type))]
    pub content_type: String,
    #[garde(custom(is_file_size))]
    pub size: u64,
}

/// A file whose bytes are about to be sent to the bucket. A device
/// names the id it made; the web names none and the server makes one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Validate)]
pub struct FileUploadNew {
    #[garde(skip)]
    #[serde(default)]
    pub id: Option<Uuid>,
    #[garde(custom(is_content_type))]
    pub content_type: String,
    #[garde(custom(is_file_size))]
    pub size: u64,
}

/// The server's answer to `FileUploadNew`: the bytes are already in the
/// bucket, or where to PUT them, with the `Content-Type` header the URL
/// was signed for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileUpload {
    pub id: Uuid,
    pub uploaded: bool,
    pub upload_url: Option<String>,
    pub content_type: String,
}

#[derive(Debug, Error)]
pub enum FileError {
    #[error("file {id} not found")]
    NotFound { id: Uuid },
    #[error("{0}")]
    Invalid(FieldErrors),
    #[error("only the owner can change a file")]
    NotOwner,
    #[error("the file did not arrive; try again")]
    NotUploaded,
    #[error("the change was refused: {0}")]
    Rejected(String),
    #[error("files are not set up on this deployment")]
    NoFileStore,
    #[error("only a device keeps a file's bytes itself")]
    DeviceOnly,
    #[error("{0}")]
    Malformed(String),
    #[error("could not read or write the file on this device: {0}")]
    Io(#[from] std::io::Error),
    #[cfg(feature = "object-storage")]
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl From<serde_json::Error> for FileError {
    fn from(error: serde_json::Error) -> FileError {
        FileError::Malformed(error.to_string())
    }
}

/// The user's files, not deleted, newest first, each with its state.
pub async fn list(deployment: &Deployment, user: &User) -> Result<Vec<FileWithState>, FileError> {
    let mut tx = deployment.begin().await?;
    let files = sql!(&mut tx, files::list_owned(user.id))?;
    let listed = with_states(&mut tx, deployment.role, files).await?;
    tx.commit().await?;
    Ok(listed)
}

/// One file of the user's, by its id, unless deleted, with its state.
pub async fn get(
    deployment: &Deployment,
    user: &User,
    id: Uuid,
) -> Result<FileWithState, FileError> {
    let mut tx = deployment.begin().await?;
    let file = get_own(&mut tx, user, id).await?;
    let mut listed = with_states(&mut tx, deployment.role, vec![file]).await?;
    tx.commit().await?;
    listed.pop().ok_or(FileError::NotFound { id })
}

/// Writes the row of a file of the user's whose bytes are already kept
/// under its id: in the bucket on the server, which refuses the row
/// otherwise, in the account's folder on a device.
pub async fn create(
    deployment: &Deployment,
    user: &User,
    new: &FileNew,
) -> Result<File, FileError> {
    let new = new.checked()?;
    let event = Event::create(
        TABLE,
        new.id,
        fields(json!({ "name": new.name, "content_type": new.content_type, "size": new.size })),
        now(),
    );
    apply(deployment, user, &event).await?.file()
}

/// Adds a file on a device: keeps its bytes in the account's folder
/// under a new id, then writes its row and queues its create, so the
/// file is there with the network off and the next sync sends it. The
/// bytes go first: a crash in between leaves bytes without a row, which
/// the cleanup removes, never a row without its bytes.
pub async fn create_with_bytes(
    deployment: &Deployment,
    user: &User,
    name: &str,
    content_type: &str,
    bytes: &[u8],
) -> Result<File, FileError> {
    if deployment.role != Role::Device {
        return Err(FileError::DeviceOnly);
    }
    let new = FileNew {
        id: Uuid::now_v7(),
        name: name.to_string(),
        content_type: content_type.to_string(),
        size: bytes.len() as u64,
    }
    .checked()?;
    let path = local_path(deployment, new.id)?;
    write_whole(&path, bytes).await?;
    match create(deployment, user, &new).await {
        Ok(file) => Ok(file),
        Err(error) => {
            delete_local(&path).await?;
            Err(error)
        }
    }
}

/// Writes a new name over the user's file.
pub async fn update(
    deployment: &Deployment,
    user: &User,
    id: Uuid,
    name: &str,
) -> Result<File, FileError> {
    let name = checked_name(name)?;
    let mut tx = deployment.begin().await?;
    let file = get_own(&mut tx, user, id).await?;
    if file.name == name {
        return Ok(file);
    }
    let event = Event::update(
        TABLE,
        id,
        file.version,
        fields(json!({ "name": file.name })),
        fields(json!({ "name": name })),
        now(),
    );
    let applied = apply_in(&mut tx, deployment.role, user, &event, true).await?;
    tx.commit().await?;
    applied.file()
}

/// Deletes the user's file: a soft delete, so it syncs like any edit,
/// and the bytes go after the row, by the sweep on the server and the
/// cleanup on a device. A file on a device that never reached the
/// server, waiting or refused, goes at once and whole, its row, its
/// events and its bytes, and nothing is pushed.
pub async fn delete(deployment: &Deployment, user: &User, id: Uuid) -> Result<(), FileError> {
    let mut tx = deployment.begin().await?;
    let file = get_own(&mut tx, user, id).await?;
    if deployment.role == Role::Device && create_of(&mut tx, id).await?.is_some() {
        events::delete_of_row(&mut tx, TABLE, id).await?;
        sql!(&mut tx, files::delete_row(id))?;
        tx.commit().await?;
        return delete_local(&local_path(deployment, id)?).await;
    }
    let event = Event::update(
        TABLE,
        id,
        file.version,
        fields(json!({ "deleted_at": null })),
        fields(json!({ "deleted_at": now() })),
        now(),
    );
    let applied = apply_in(&mut tx, deployment.role, user, &event, true).await?;
    tx.commit().await?;
    applied.file().map(|_| ())
}

/// Applies an event of the user's to its file and answers it: what the
/// server does with a pushed event, and what a local write does on any
/// deployment.
pub async fn apply_event(
    deployment: &Deployment,
    user: &User,
    event: &Event,
) -> Result<Answer, FileError> {
    Ok(apply(deployment, user, event).await?.answer)
}

/// Answers where to PUT a file's bytes, or that the bucket already has
/// them: on the server, which holds the bucket's keys. The file has no
/// row yet, so what holds the user to their own files is the key's
/// prefix.
#[cfg(feature = "object-storage")]
pub async fn create_upload(
    deployment: &Deployment,
    user: &User,
    request: &FileUploadNew,
) -> Result<FileUpload, FileError> {
    let storage = bucket(deployment)?;
    let content_type = content_type_or_unknown(&request.content_type);
    let request = FileUploadNew {
        content_type: content_type.clone(),
        ..request.clone()
    };
    FieldErrors::check(&request).map_err(FileError::Invalid)?;
    let id = request.id.unwrap_or_else(Uuid::now_v7);
    let key = storage_key(user.id, id);
    if storage.size(&key).await? == Some(request.size) {
        return Ok(FileUpload {
            id,
            uploaded: true,
            upload_url: None,
            content_type,
        });
    }
    let upload_url = storage
        .upload_url(&key, &content_type, UPLOAD_URL_LIFETIME)
        .await?;
    Ok(FileUpload {
        id,
        uploaded: false,
        upload_url: Some(upload_url),
        content_type,
    })
}

/// A URL the user's file is read from for the next hour, signed by the
/// server; as an attachment, the bucket answers it to be saved under
/// the file's name instead of shown.
#[cfg(feature = "object-storage")]
pub async fn content_url(
    deployment: &Deployment,
    user: &User,
    id: Uuid,
    as_attachment: bool,
) -> Result<String, FileError> {
    let storage = bucket(deployment)?;
    let mut tx = deployment.begin().await?;
    let file = get_own(&mut tx, user, id).await?;
    tx.commit().await?;
    let content_disposition = as_attachment.then(|| attachment_named(&file.name));
    Ok(storage
        .download_url(
            &storage_key(file.owner_id, file.id),
            DOWNLOAD_URL_LIFETIME,
            content_disposition.as_deref(),
        )
        .await?)
}

/// A file waiting on a device, with what the server said when asked
/// about its bytes: nothing when it could not be asked or would not
/// say, that it has them, or where to send them.
#[derive(Clone, Debug)]
pub struct WaitingFile {
    pub id: Uuid,
    /// The file's create, pending in the queue.
    pub create_id: Uuid,
    pub upload: Option<FileUpload>,
}

/// The first step of a sync on a device: asks the server about the
/// bytes of every file whose create is pending. What the device knows
/// of an upload is only this answer, asked again by every sync and
/// kept nowhere. Once the server cannot be reached the rest are not
/// asked, so a device offline waits for one request, not one per file.
pub async fn ask_about_waiting<T: Transport>(
    deployment: &Deployment,
    transport: &T,
    token: Option<&str>,
) -> Result<Vec<WaitingFile>, SyncError> {
    let mut tx = deployment.begin().await?;
    let creates: Vec<StoredEvent> = events::pending(&mut tx)
        .await?
        .into_iter()
        .filter(|stored| stored.event.table == TABLE && stored.event.kind == EventKind::Create)
        .collect();
    tx.commit().await?;

    let mut waiting = Vec::with_capacity(creates.len());
    let mut reachable = true;
    for stored in creates {
        let event = &stored.event;
        let mut file = WaitingFile {
            id: event.row_id,
            create_id: event.id,
            upload: None,
        };
        let request = FileUploadNew {
            id: Some(event.row_id),
            content_type: event.changes["content_type"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            size: event.changes["size"].as_u64().unwrap_or_default(),
        };
        if reachable {
            match transport.files_create_upload(token, request).await {
                Ok(upload) => file.upload = Some(upload),
                Err(SyncError::Refused(reason)) => {
                    note_error(deployment, event.id, &reason).await?;
                }
                Err(error @ SyncError::Transport(_)) => {
                    reachable = false;
                    note_error(deployment, event.id, &error.to_string()).await?;
                }
                Err(error) => return Err(error),
            }
        }
        waiting.push(file);
    }
    Ok(waiting)
}

/// The files whose bytes the server does not have: their events are
/// held back from the push.
pub fn not_uploaded(waiting: &[WaitingFile]) -> BTreeSet<Uuid> {
    waiting
        .iter()
        .filter(|file| !file.upload.as_ref().is_some_and(|upload| upload.uploaded))
        .map(|file| file.id)
        .collect()
}

/// The last step of a sync on a device before the files' events are
/// pushed: sends the bytes of each waiting file the server gave a URL
/// for, from the account's folder, and marks it uploaded. How many
/// went. A file that fails keeps its reason and waits for the next
/// sync; once the bucket cannot be reached the rest wait too.
///
/// This runs inside the sync, which holds the sync lock while bytes go.
/// An application with large files calls it from a task of its own
/// instead, followed by a sync.
pub async fn upload_waiting<T: Transport>(
    deployment: &Deployment,
    transport: &T,
    token: Option<&str>,
    waiting: &mut [WaitingFile],
) -> Result<usize, SyncError> {
    let mut uploaded = 0;
    for file in waiting {
        let Some(upload) = file.upload.as_mut().filter(|upload| !upload.uploaded) else {
            continue;
        };
        let path = local_path(deployment, file.id)?;
        match transport.files_upload(token, upload, &path).await {
            Ok(()) => {
                upload.uploaded = true;
                uploaded += 1;
            }
            Err(SyncError::Refused(reason)) => {
                note_error(deployment, file.create_id, &reason).await?;
            }
            Err(error @ SyncError::Transport(_)) => {
                note_error(deployment, file.create_id, &error.to_string()).await?;
                break;
            }
            Err(error) => return Err(error),
        }
    }
    Ok(uploaded)
}

/// Keeps why a waiting file's bytes did not go, for the pages.
async fn note_error(
    deployment: &Deployment,
    create_id: Uuid,
    reason: &str,
) -> Result<(), SyncError> {
    let mut tx = deployment.begin().await?;
    events::mark_error(&mut tx, create_id, reason).await?;
    tx.commit().await?;
    Ok(())
}

/// The file as it travels on the wire.
pub fn table_row(file: &File) -> Result<TableRow, FileError> {
    Ok(TableRow {
        table: TABLE.to_string(),
        row: serde_json::to_value(file)?,
    })
}

/// Where a file's bytes are in the bucket.
pub fn storage_key(owner_id: Uuid, id: Uuid) -> String {
    format!("{KEY_PREFIX}/{owner_id}/{id}")
}

/// Writes the server's row over the device's, unless an event of the
/// device's for it is still pending; true when it was written.
pub(crate) async fn apply_server_row(tx: &mut Transaction, row: &Value) -> Result<bool, FileError> {
    let file: File = serde_json::from_value(row.clone())?;
    if events::has_pending(tx, TABLE, file.id).await? {
        return Ok(false);
    }
    sql!(&mut *tx, files::upsert(&file))?;
    Ok(true)
}

/// Removes a device's row the server holds under another id.
pub(crate) async fn delete_row(tx: &mut Transaction, id: Uuid) -> Result<(), FileError> {
    Ok(sql!(tx, files::delete_row(id))?)
}

/// A page of the user's rows the server wrote after the cursor, deleted
/// or not.
pub(crate) async fn list_synced_after(
    tx: &mut Transaction,
    user: &User,
    after: &Cursor,
    limit: usize,
) -> Result<Vec<File>, FileError> {
    Ok(sql!(tx, files::list_synced_after(user.id, after, limit))?)
}

/// The row by its id, deleted or not, whoever owns it.
pub(crate) async fn get_in(tx: &mut Transaction, id: Uuid) -> Result<Option<File>, FileError> {
    Ok(sql!(tx, files::get(id))?)
}

/// Where a file's bytes are on this device, there or not.
pub fn local_path(deployment: &Deployment, id: Uuid) -> Result<PathBuf, FileError> {
    match &deployment.file_store {
        FileStore::Folder(folder) => Ok(folder.join(id.hyphenated().to_string())),
        _ => Err(FileError::NoFileStore),
    }
}

/// Writes the bytes under a temporary name beside the path, then
/// renames, so a file at its path is always whole.
async fn write_whole(path: &Path, bytes: &[u8]) -> Result<(), FileError> {
    let partial = partial_path(path);
    tokio::fs::write(&partial, bytes).await?;
    tokio::fs::rename(&partial, path).await?;
    Ok(())
}

/// The name a file is written under until it is whole.
pub(crate) fn partial_path(path: &Path) -> PathBuf {
    path.with_extension("part")
}

/// Removes the bytes at the path, if any are there.
async fn delete_local(path: &Path) -> Result<(), FileError> {
    match tokio::fs::remove_file(path).await {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

/// The create of a file that has not reached the server, on a device:
/// pending while the file waits, a conflict once the server refused it.
async fn create_of(tx: &mut Transaction, id: Uuid) -> Result<Option<StoredEvent>, FileError> {
    Ok(events::list_of_table(tx, TABLE)
        .await?
        .into_iter()
        .find(|stored| stored.event.row_id == id && stored.event.kind == EventKind::Create))
}

/// The live file, if the user owns it; another user's is not found, as
/// if it were not there.
async fn get_own(tx: &mut Transaction, user: &User, id: Uuid) -> Result<File, FileError> {
    match get_in(tx, id).await? {
        Some(file) if file.deleted_at.is_none() && file.owner_id == user.id => Ok(file),
        _ => Err(FileError::NotFound { id }),
    }
}

/// The files with their states. On the server every file is synced; on
/// a device a file whose create waits in the queue is waiting, and one
/// whose create the server refused is refused.
async fn with_states(
    tx: &mut Transaction,
    role: Role,
    files: Vec<File>,
) -> Result<Vec<FileWithState>, FileError> {
    let creates: Vec<StoredEvent> = match role {
        Role::Server => Vec::new(),
        Role::Device => events::list_of_table(tx, TABLE)
            .await?
            .into_iter()
            .filter(|stored| stored.event.kind == EventKind::Create)
            .collect(),
    };
    Ok(files
        .into_iter()
        .map(|file| {
            let create = creates.iter().find(|stored| stored.event.row_id == file.id);
            let (state, error) = match create {
                Some(stored) if stored.status == EventStatus::Pending => {
                    (FileState::Waiting, stored.last_error.clone())
                }
                Some(stored) => (FileState::Refused, stored.last_error.clone()),
                None => (FileState::Synced, None),
            };
            FileWithState {
                shown_as: ShownAs::of(&file.content_type),
                file,
                state,
                error,
            }
        })
        .collect())
}

/// An event applied: the answer, and the file as it now stands.
pub(crate) struct Applied {
    pub answer: Answer,
    pub file: Option<File>,
}

impl Applied {
    fn file(self) -> Result<File, FileError> {
        match (self.answer.status, self.file) {
            (AnswerStatus::Rejected, _) => {
                let reason = self.answer.reason.unwrap_or_else(|| "rejected".to_string());
                Err(if reason == FILE_NOT_UPLOADED {
                    FileError::NotUploaded
                } else {
                    FileError::Rejected(reason)
                })
            }
            (_, Some(file)) => Ok(file),
            (_, None) => Err(FileError::NotFound { id: self.answer.id }),
        }
    }
}

/// Applies the event in a transaction of its own. The server looks in
/// its bucket first when the event creates a file, outside the
/// transaction, since the bucket is another machine.
async fn apply(deployment: &Deployment, user: &User, event: &Event) -> Result<Applied, FileError> {
    let uploaded = match (deployment.role, event.kind) {
        (Role::Server, EventKind::Create) => is_uploaded(deployment, user, event).await?,
        _ => true,
    };
    let mut tx = deployment.begin().await?;
    let applied = apply_in(&mut tx, deployment.role, user, event, uploaded).await?;
    tx.commit().await?;
    Ok(applied)
}

/// Whether the bucket holds the bytes of the file the event creates, at
/// the size the event says.
async fn is_uploaded(
    deployment: &Deployment,
    user: &User,
    event: &Event,
) -> Result<bool, FileError> {
    let Some(size) = event.changes.get("size").and_then(Value::as_u64) else {
        return Ok(false);
    };
    let key = storage_key(user.id, event.row_id);
    Ok(bucket_size(deployment, &key).await? == Some(size))
}

#[cfg(feature = "object-storage")]
async fn bucket_size(deployment: &Deployment, key: &str) -> Result<Option<u64>, FileError> {
    Ok(bucket(deployment)?.size(key).await?)
}

/// Without the bucket's code compiled in, as in the apps, there is none.
#[cfg(not(feature = "object-storage"))]
async fn bucket_size(_deployment: &Deployment, _key: &str) -> Result<Option<u64>, FileError> {
    Err(FileError::NoFileStore)
}

/// Applies the event in the transaction: checks the fields it changes
/// and that the user owns the file, merges it into the row, writes the
/// row, and then the role's part, the record of the answer on the server
/// or the queue on a device. A create whose bytes are not `uploaded` is
/// answered `file_not_uploaded` and writes nothing.
pub(crate) async fn apply_in(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    event: &Event,
    uploaded: bool,
) -> Result<Applied, FileError> {
    if role == Role::Server
        && let Some(recorded) = events::get(tx, event.id).await?
    {
        let file = get_in(tx, event.row_id).await?;
        let server = file.as_ref().map(table_row).transpose()?;
        return Ok(Applied {
            answer: recorded.answer(server),
            file,
        });
    }

    let allowed = match event.kind {
        EventKind::Create => CREATE_FIELDS,
        EventKind::Update => UPDATE_FIELDS,
    };
    if let Some(reason) = events::not_editable(&event.changes, allowed) {
        let file = get_in(tx, event.row_id).await?;
        return rejected(event, reason, file);
    }

    let now = now();
    match event.kind {
        EventKind::Create => {
            if let Some(existing) = get_in(tx, event.row_id).await? {
                // Another user's row is none of this user's business
                let existing = (existing.owner_id == user.id).then_some(existing);
                return rejected(event, "exists", existing);
            }
            let (version, last_synced_at) = match role {
                Role::Server => (1, Some(now)),
                Role::Device => (0, None),
            };
            let blank = File {
                id: event.row_id,
                owner_id: user.id,
                name: String::new(),
                content_type: String::new(),
                size: 0,
                inserted_at: now,
                updated_at: now,
                version,
                last_synced_at,
                deleted_at: None,
                deleted_by_id: None,
            };
            let file = match blank.with_fields(&event.changes) {
                Ok(file) => file,
                Err(error) => return rejected(event, format!("invalid: {error}"), None),
            };
            if !uploaded {
                return rejected(event, FILE_NOT_UPLOADED, None);
            }
            sql!(&mut *tx, files::insert(&file))?;
            finish(tx, role, user, event, Conflicts::new(), file).await
        }
        EventKind::Update => {
            let Some(file) = get_in(tx, event.row_id).await? else {
                return rejected(event, "unknown_file", None);
            };
            if file.owner_id != user.id {
                return rejected(event, "not_owner", None);
            }
            let (changes, conflicts) = events::merge(
                &file.fields(UPDATE_FIELDS)?,
                file.version,
                event.base_version,
                &event.base,
                &event.changes,
            );
            let file = if changes.is_empty() {
                file
            } else {
                let mut updated = match file.with_fields(&changes) {
                    Ok(updated) => updated,
                    Err(error) => return rejected(event, format!("invalid: {error}"), Some(file)),
                };
                updated.updated_at = now;
                if changes.contains_key("deleted_at") {
                    updated.deleted_by_id = updated.deleted_at.map(|_| user.id);
                }
                if role == Role::Server {
                    updated.version += 1;
                    updated.last_synced_at = Some(now);
                }
                sql!(&mut *tx, files::update(&updated))?;
                updated
            };
            finish(tx, role, user, event, conflicts, file).await
        }
    }
}

/// Answers an applied event and does the role's part.
async fn finish(
    tx: &mut Transaction,
    role: Role,
    user: &User,
    event: &Event,
    conflicts: Conflicts,
    file: File,
) -> Result<Applied, FileError> {
    let answer = Answer::applied(event.id, file.version, conflicts, table_row(&file)?);
    match role {
        Role::Server => events::record(tx, user.id, event, &answer).await?,
        Role::Device => events::queue(tx, user.id, event).await?,
    }
    Ok(Applied {
        answer,
        file: Some(file),
    })
}

fn rejected(
    event: &Event,
    reason: impl Into<String>,
    file: Option<File>,
) -> Result<Applied, FileError> {
    let server = file.as_ref().map(table_row).transpose()?;
    Ok(Applied {
        answer: Answer::rejected(event.id, reason, server),
        file,
    })
}

impl File {
    /// The named fields as they travel on the wire.
    pub(crate) fn fields(&self, names: &[&str]) -> Result<Fields, FileError> {
        let mut all = fields(serde_json::to_value(self)?);
        all.retain(|name, _| names.contains(&name.as_str()));
        Ok(all)
    }

    /// The file with the fields written over it, checked.
    fn with_fields(&self, changes: &Fields) -> Result<File, FileError> {
        let mut all = fields(serde_json::to_value(self)?);
        for (name, value) in changes {
            all.insert(name.clone(), value.clone());
        }
        let file: File = serde_json::from_value(Value::Object(all))?;
        let checked = FileNew {
            id: file.id,
            name: file.name.clone(),
            content_type: file.content_type.clone(),
            size: file.size,
        }
        .checked()?;
        Ok(File {
            name: checked.name,
            content_type: checked.content_type,
            ..file
        })
    }
}

impl FileNew {
    /// The file as it is kept: its name trimmed, a missing content type
    /// filled in, and every field within the rules.
    pub fn checked(&self) -> Result<FileNew, FileError> {
        let checked = FileNew {
            id: self.id,
            name: self.name.trim().to_string(),
            content_type: content_type_or_unknown(&self.content_type),
            size: self.size,
        };
        FieldErrors::check(&checked).map_err(FileError::Invalid)?;
        Ok(checked)
    }
}

/// The server's bucket, where there is one.
#[cfg(feature = "object-storage")]
pub(crate) fn bucket(deployment: &Deployment) -> Result<&Storage, FileError> {
    match &deployment.file_store {
        FileStore::Bucket(storage) => Ok(storage),
        _ => Err(FileError::NoFileStore),
    }
}

/// The fields of a JSON object, or none for anything else.
fn fields(value: Value) -> Fields {
    match value {
        Value::Object(fields) => fields,
        _ => Fields::new(),
    }
}

/// The name as it is kept: trimmed, not blank, not too long.
fn checked_name(name: &str) -> Result<String, FileError> {
    let name = name.trim();
    is_file_name(name, &()).map_err(|error| {
        FileError::Invalid(FieldErrors::one("name", error.message().to_string()))
    })?;
    Ok(name.to_string())
}

/// The content type a browser gave, or the unknown one when it gave none.
fn content_type_or_unknown(content_type: &str) -> String {
    match content_type.trim() {
        "" => UNKNOWN_CONTENT_TYPE.to_string(),
        content_type => content_type.to_lowercase(),
    }
}

fn is_file_name(value: &str, _context: &()) -> garde::Result {
    if value.trim().is_empty() {
        return Err(garde::Error::new("is blank"));
    }
    if value.chars().count() > MAX_NAME_LENGTH {
        return Err(garde::Error::new("is longer than 255 characters"));
    }
    Ok(())
}

/// A `type/subtype`, with parameters or not, in printable ASCII.
fn is_content_type(value: &str, _context: &()) -> garde::Result {
    let essence = value.split(';').next().unwrap_or_default().trim();
    let well_formed = essence.split_once('/').is_some_and(|(kind, subtype)| {
        !kind.is_empty() && !subtype.is_empty() && !subtype.contains('/')
    });
    let printable = value.chars().all(|c| c.is_ascii_graphic() || c == ' ');
    if !well_formed || !printable || value.len() > MAX_NAME_LENGTH {
        return Err(garde::Error::new("is not a content type"));
    }
    Ok(())
}

fn is_file_size(value: &u64, _context: &()) -> garde::Result {
    if *value == 0 {
        return Err(garde::Error::new("is empty"));
    }
    if *value > MAX_BYTES {
        return Err(garde::Error::new("is larger than 25 MB"));
    }
    Ok(())
}

/// The `Content-Disposition` that has a browser save a file under the
/// name, which travels percent-encoded so any name is safe in a header.
#[cfg(feature = "object-storage")]
fn attachment_named(name: &str) -> String {
    let mut encoded = String::new();
    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    format!("attachment; filename*=UTF-8''{encoded}")
}

#[cfg(all(test, feature = "object-storage"))]
pub(crate) mod tests {
    use super::*;
    use crate::accounts::Session;
    use crate::database::{Database, with_each_database};
    use crate::sync::tests::user_with_token;
    use crate::sync::{
        self, BackfillRequest, InProcess, Page, PullRequest, PushRequest, PushResponse,
    };
    use crate::{posts, users};

    /// A server with a bucket in memory.
    pub(crate) fn server(database: Database) -> (Deployment, Storage) {
        let storage = Storage::memory();
        let deployment = Deployment::new(database, Role::Server)
            .with_file_store(FileStore::Bucket(storage.clone()));
        (deployment, storage)
    }

    pub(crate) fn new_file(name: &str, bytes: &[u8]) -> FileNew {
        FileNew {
            id: Uuid::now_v7(),
            name: name.to_string(),
            content_type: "text/plain".to_string(),
            size: bytes.len() as u64,
        }
    }

    /// A file of the user's on the server, its bytes put in the bucket
    /// as an upload would have.
    pub(crate) async fn uploaded(
        deployment: &Deployment,
        storage: &Storage,
        user: &User,
        name: &str,
        bytes: &[u8],
    ) -> File {
        let new = new_file(name, bytes);
        storage
            .put(&storage_key(user.id, new.id), bytes.to_vec())
            .await
            .expect("the upload");
        create(deployment, user, &new).await.expect("a file")
    }

    /// A device the user is signed in on, with a folder for its files'
    /// bytes, which goes when the test drops it.
    pub(crate) async fn device_with_folder(
        user: &User,
        token: &str,
    ) -> (Deployment, tempfile::TempDir) {
        let folder = tempfile::tempdir().expect("a folder");
        let deployment = users::tests::device_of(user, token)
            .await
            .with_file_store(FileStore::Folder(folder.path().to_path_buf()));
        (deployment, folder)
    }

    #[tokio::test]
    async fn files_create_with_bytes_on_a_device_keeps_them_and_waits() {
        let alice = users::tests::alice();
        let (device, folder) = device_with_folder(&alice, "a token").await;

        let file = create_with_bytes(&device, &alice, " notes.txt ", "text/plain", b"some notes")
            .await
            .expect("a file");
        assert_eq!(file.name, "notes.txt");
        assert_eq!(file.size, 10);
        assert_eq!(file.version, 0);
        let path = local_path(&device, file.id).expect("a path");
        assert_eq!(std::fs::read(&path).expect("the bytes"), b"some notes");
        let listed = list(&device, &alice).await.expect("the files");
        assert_eq!(listed[0].file, file);
        assert_eq!(listed[0].state, FileState::Waiting);

        let error = create_with_bytes(&device, &alice, "empty.txt", "text/plain", b"")
            .await
            .expect_err("no bytes");
        assert!(matches!(error, FileError::Invalid(_)));
        assert_eq!(
            std::fs::read_dir(folder.path())
                .expect("the folder")
                .count(),
            1,
            "a file outside the rules leaves no bytes"
        );
    }

    #[tokio::test]
    async fn files_delete_of_a_waiting_file_on_a_device_leaves_nothing() {
        let alice = users::tests::alice();
        let (device, folder) = device_with_folder(&alice, "a token").await;
        let file = create_with_bytes(&device, &alice, "notes.txt", "text/plain", b"x")
            .await
            .expect("a file");
        update(&device, &alice, file.id, "renamed.txt")
            .await
            .expect("renamed while waiting");

        delete(&device, &alice, file.id).await.expect("deleted");
        assert!(list(&device, &alice).await.expect("the files").is_empty());
        let mut tx = device.begin().await.expect("a transaction");
        assert!(get_in(&mut tx, file.id).await.expect("looked").is_none());
        assert!(
            events::list_of_table(&mut tx, TABLE)
                .await
                .expect("the events")
                .is_empty(),
            "nothing is left to push"
        );
        assert_eq!(
            std::fs::read_dir(folder.path())
                .expect("the folder")
                .count(),
            0
        );
    }

    /// The server in this process, with the faults a test asks for.
    struct Faulty<'a> {
        server: InProcess<'a>,
        /// How many pushes carrying a file's event are lost on the way.
        file_pushes_lost: std::sync::Mutex<u32>,
        /// Answers that the bucket has every file's bytes, asked or not.
        says_uploaded: bool,
    }

    impl<'a> Faulty<'a> {
        fn new(server: &'a Deployment) -> Faulty<'a> {
            Faulty {
                server: InProcess { server },
                file_pushes_lost: std::sync::Mutex::new(0),
                says_uploaded: false,
            }
        }
    }

    impl Transport for Faulty<'_> {
        async fn push(
            &self,
            token: Option<&str>,
            request: PushRequest,
        ) -> Result<PushResponse, SyncError> {
            let carries_a_file = request.events.iter().any(|event| event.table == TABLE);
            {
                let mut lost = self.file_pushes_lost.lock().expect("the lock");
                if carries_a_file && *lost > 0 {
                    *lost -= 1;
                    return Err(SyncError::Transport("the connection dropped".to_string()));
                }
            }
            self.server.push(token, request).await
        }

        async fn pull(&self, token: Option<&str>, request: PullRequest) -> Result<Page, SyncError> {
            self.server.pull(token, request).await
        }

        async fn backfill(
            &self,
            token: Option<&str>,
            request: BackfillRequest,
        ) -> Result<Page, SyncError> {
            self.server.backfill(token, request).await
        }

        async fn sign_in_with_device_code(
            &self,
            code: &str,
            verifier: &str,
        ) -> Result<Session, SyncError> {
            self.server.sign_in_with_device_code(code, verifier).await
        }

        async fn sign_out(&self, token: &str) -> Result<(), SyncError> {
            self.server.sign_out(token).await
        }

        async fn files_create_upload(
            &self,
            token: Option<&str>,
            request: FileUploadNew,
        ) -> Result<FileUpload, SyncError> {
            let upload = self.server.files_create_upload(token, request).await?;
            Ok(FileUpload {
                uploaded: upload.uploaded || self.says_uploaded,
                ..upload
            })
        }

        async fn files_upload(
            &self,
            token: Option<&str>,
            upload: &FileUpload,
            path: &Path,
        ) -> Result<(), SyncError> {
            self.server.files_upload(token, upload, path).await
        }

        async fn files_download(
            &self,
            token: Option<&str>,
            id: Uuid,
            path: &Path,
        ) -> Result<(), SyncError> {
            self.server.files_download(token, id, path).await
        }
    }

    async fn state_of(
        deployment: &Deployment,
        user: &User,
        id: Uuid,
    ) -> (FileState, Option<String>) {
        let shown = get(deployment, user, id).await.expect("the file");
        (shown.state, shown.error)
    }

    async fn in_bucket(storage: &Storage, user: &User, id: Uuid) -> Option<u64> {
        storage
            .size(&storage_key(user.id, id))
            .await
            .expect("looked")
    }

    #[tokio::test]
    async fn files_added_offline_reach_the_bucket_the_server_and_another_device() {
        with_each_database(|database| async move {
            let (server, storage) = server(database);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let (a, _a_folder) = device_with_folder(&alice, &token).await;
            let (b, _b_folder) = device_with_folder(&alice, &token).await;
            let file = create_with_bytes(&a, &alice, "notes.txt", "text/plain", b"some notes")
                .await
                .expect("a file");
            posts::create(&a, &alice, "a post beside it", &[])
                .await
                .expect("a post");
            assert_eq!(in_bucket(&storage, &alice, file.id).await, None);

            let report = sync::run(&a, &InProcess { server: &server })
                .await
                .expect("a sync");
            assert_eq!(report.uploaded, 1);
            assert_eq!(
                report.pushed, 2,
                "the post, then the file once its bytes went"
            );
            assert_eq!(report.pending, 0);
            assert_eq!(report.push_error, None);
            assert_eq!(in_bucket(&storage, &alice, file.id).await, Some(10));
            let on_server = get(&server, &alice, file.id).await.expect("on the server");
            assert_eq!(on_server.file.name, "notes.txt");
            assert_eq!(on_server.file.version, 1);
            let on_a = get(&a, &alice, file.id).await.expect("on a");
            assert_eq!(on_a.state, FileState::Synced);
            assert_eq!(on_a.file, on_server.file, "the server's row came back");
            assert!(
                local_path(&a, file.id).expect("a path").exists(),
                "the device that added it keeps its copy"
            );

            sync::run(&b, &InProcess { server: &server })
                .await
                .expect("a sync");
            let on_b = get(&b, &alice, file.id).await.expect("on b");
            assert_eq!(on_b.file, on_server.file);
            assert_eq!(on_b.state, FileState::Synced);
        })
        .await;
    }

    #[tokio::test]
    async fn files_a_failed_upload_holds_back_only_its_own_events() {
        with_each_database(|database| async move {
            let (server, storage) = server(database);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let (a, _folder) = device_with_folder(&alice, &token).await;
            let lost = create_with_bytes(&a, &alice, "lost.txt", "text/plain", b"lost")
                .await
                .expect("a file");
            let kept = create_with_bytes(&a, &alice, "kept.txt", "text/plain", b"kept")
                .await
                .expect("a file");
            update(&a, &alice, lost.id, "renamed.txt")
                .await
                .expect("renamed while waiting");
            posts::create(&a, &alice, "a post", &[])
                .await
                .expect("a post");
            std::fs::remove_file(local_path(&a, lost.id).expect("a path")).expect("its bytes gone");

            let report = sync::run(&a, &InProcess { server: &server })
                .await
                .expect("a sync");
            assert_eq!(report.uploaded, 1);
            assert_eq!(report.pushed, 2, "the post and the other file");
            assert_eq!(
                report.pending, 2,
                "the file's create and its rename wait together"
            );
            assert_eq!(in_bucket(&storage, &alice, kept.id).await, Some(4));
            assert_eq!(in_bucket(&storage, &alice, lost.id).await, None);
            let on_server: Vec<String> = list(&server, &alice)
                .await
                .expect("the files")
                .into_iter()
                .map(|listed| listed.file.name)
                .collect();
            assert_eq!(on_server, ["kept.txt"]);
            let (state, error) = state_of(&a, &alice, lost.id).await;
            assert_eq!(state, FileState::Waiting);
            assert!(error.is_some(), "why it did not go is kept for the page");
            assert_eq!(state_of(&a, &alice, kept.id).await.0, FileState::Synced);
        })
        .await;
    }

    #[tokio::test]
    async fn files_a_sync_cut_off_after_the_upload_is_finished_by_the_next() {
        with_each_database(|database| async move {
            let (server, storage) = server(database);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let (a, _folder) = device_with_folder(&alice, &token).await;
            let file = create_with_bytes(&a, &alice, "notes.txt", "text/plain", b"some notes")
                .await
                .expect("a file");
            let transport = Faulty::new(&server);
            *transport.file_pushes_lost.lock().expect("the lock") = 1;

            let report = sync::run(&a, &transport).await.expect("a sync");
            assert_eq!(report.uploaded, 1);
            assert!(report.push_error.is_some());
            assert_eq!(in_bucket(&storage, &alice, file.id).await, Some(10));
            assert_eq!(state_of(&a, &alice, file.id).await.0, FileState::Waiting);

            let report = sync::run(&a, &transport).await.expect("a sync");
            assert_eq!(report.uploaded, 0, "the server said it has the bytes");
            assert_eq!(report.pushed, 1);
            assert_eq!(state_of(&a, &alice, file.id).await.0, FileState::Synced);
            assert!(get(&server, &alice, file.id).await.is_ok());
        })
        .await;
    }

    #[tokio::test]
    async fn files_bytes_gone_from_the_bucket_before_the_push_are_sent_again() {
        with_each_database(|database| async move {
            let (server, storage) = server(database);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let (a, _folder) = device_with_folder(&alice, &token).await;
            let file = create_with_bytes(&a, &alice, "notes.txt", "text/plain", b"some notes")
                .await
                .expect("a file");
            let transport = Faulty::new(&server);
            *transport.file_pushes_lost.lock().expect("the lock") = 1;
            sync::run(&a, &transport).await.expect("a sync cut off");
            storage
                .delete(&storage_key(alice.id, file.id))
                .await
                .expect("swept, as a file no row points at is after a day");

            let report = sync::run(&a, &transport).await.expect("a sync");
            assert_eq!(report.uploaded, 1);
            assert_eq!(in_bucket(&storage, &alice, file.id).await, Some(10));
            assert_eq!(state_of(&a, &alice, file.id).await.0, FileState::Synced);
        })
        .await;
    }

    #[tokio::test]
    async fn files_a_create_pushed_without_its_bytes_stays_waiting_and_is_tried_again() {
        with_each_database(|database| async move {
            let (server, _storage) = server(database);
            let (alice, token) = user_with_token(&server, "alice@example.com").await;
            let (a, _folder) = device_with_folder(&alice, &token).await;
            let file = create_with_bytes(&a, &alice, "notes.txt", "text/plain", b"some notes")
                .await
                .expect("a file");
            let wrong = Faulty {
                says_uploaded: true,
                ..Faulty::new(&server)
            };

            let report = sync::run(&a, &wrong).await.expect("a sync");
            assert_eq!(report.rejected, 1);
            assert_eq!(report.pending, 1);
            let (state, error) = state_of(&a, &alice, file.id).await;
            assert_eq!(state, FileState::Waiting, "the file is not dropped");
            assert_eq!(error.as_deref(), Some(FILE_NOT_UPLOADED));
            assert!(get(&server, &alice, file.id).await.is_err());

            sync::run(&a, &InProcess { server: &server })
                .await
                .expect("a sync");
            assert_eq!(state_of(&a, &alice, file.id).await.0, FileState::Synced);
        })
        .await;
    }

    #[tokio::test]
    async fn files_create_needs_the_bytes_in_the_bucket() {
        with_each_database(|database| async move {
            let (deployment, storage) = server(database);
            let alice = users::create(&deployment, "alice@example.com")
                .await
                .expect("a user");
            let new = new_file("notes.txt", b"some notes");

            let error = create(&deployment, &alice, &new)
                .await
                .expect_err("no bytes yet");
            assert!(matches!(error, FileError::NotUploaded));
            assert!(
                list(&deployment, &alice)
                    .await
                    .expect("the files")
                    .is_empty()
            );

            storage
                .put(&storage_key(alice.id, new.id), b"other".to_vec())
                .await
                .expect("an upload of another size");
            let error = create(&deployment, &alice, &new)
                .await
                .expect_err("not the size it said");
            assert!(matches!(error, FileError::NotUploaded));

            storage
                .put(&storage_key(alice.id, new.id), b"some notes".to_vec())
                .await
                .expect("the upload");
            let file = create(&deployment, &alice, &new).await.expect("a file");
            assert_eq!(file.id, new.id);
            assert_eq!(file.owner_id, alice.id);
            assert_eq!(file.size, 10);
            assert_eq!(file.version, 1);
            let listed = list(&deployment, &alice).await.expect("the files");
            assert_eq!(listed.len(), 1);
            assert_eq!(listed[0].file, file);
            assert_eq!(listed[0].state, FileState::Synced);
            assert_eq!(listed[0].shown_as, ShownAs::Text);
            assert_eq!(ShownAs::of("image/gif"), ShownAs::Image);
            assert_eq!(ShownAs::of("application/pdf"), ShownAs::Pdf);
            assert_eq!(ShownAs::of("text/html"), ShownAs::NotShown);
        })
        .await;
    }

    #[tokio::test]
    async fn files_create_trims_the_name_and_refuses_what_is_outside_the_rules() {
        with_each_database(|database| async move {
            let (deployment, storage) = server(database);
            let alice = users::create(&deployment, "alice@example.com")
                .await
                .expect("a user");
            let file = uploaded(&deployment, &storage, &alice, "  spaced.txt ", b"x").await;
            assert_eq!(file.name, "spaced.txt");

            let wrong = FileNew {
                id: Uuid::now_v7(),
                name: "  ".to_string(),
                content_type: "not a type".to_string(),
                size: MAX_BYTES + 1,
            };
            let error = create(&deployment, &alice, &wrong)
                .await
                .expect_err("three fields wrong");
            let FileError::Invalid(errors) = error else {
                panic!("field errors, got {error}");
            };
            assert_eq!(errors.0["name"], ["is blank"]);
            assert_eq!(errors.0["content_type"], ["is not a content type"]);
            assert_eq!(errors.0["size"], ["is larger than 25 MB"]);

            let untyped = FileNew {
                content_type: String::new(),
                ..new_file("data", b"x")
            };
            storage
                .put(&storage_key(alice.id, untyped.id), b"x".to_vec())
                .await
                .expect("the upload");
            let file = create(&deployment, &alice, &untyped)
                .await
                .expect("a file of no known type");
            assert_eq!(file.content_type, UNKNOWN_CONTENT_TYPE);
        })
        .await;
    }

    #[tokio::test]
    async fn files_list_and_get_show_a_user_their_own_files_only() {
        with_each_database(|database| async move {
            let (deployment, storage) = server(database);
            let alice = users::create(&deployment, "alice@example.com")
                .await
                .expect("a user");
            let bob = users::create(&deployment, "bob@example.com")
                .await
                .expect("a user");
            let first = uploaded(&deployment, &storage, &alice, "first.txt", b"1").await;
            let second = uploaded(&deployment, &storage, &alice, "second.txt", b"2").await;

            let names: Vec<String> = list(&deployment, &alice)
                .await
                .expect("the files")
                .into_iter()
                .map(|listed| listed.file.name)
                .collect();
            assert_eq!(names, ["second.txt", "first.txt"]);
            assert_eq!(
                get(&deployment, &alice, first.id).await.expect("hers").file,
                first
            );
            assert!(list(&deployment, &bob).await.expect("the files").is_empty());
            for error in [
                get(&deployment, &bob, second.id)
                    .await
                    .expect_err("not his"),
                update(&deployment, &bob, second.id, "his.txt")
                    .await
                    .expect_err("not his"),
                delete(&deployment, &bob, second.id)
                    .await
                    .expect_err("not his"),
                content_url(&deployment, &bob, second.id, false)
                    .await
                    .expect_err("not his"),
            ] {
                assert!(matches!(error, FileError::NotFound { .. }), "{error}");
            }
        })
        .await;
    }

    #[tokio::test]
    async fn files_update_changes_the_name_only() {
        with_each_database(|database| async move {
            let (deployment, storage) = server(database);
            let alice = users::create(&deployment, "alice@example.com")
                .await
                .expect("a user");
            let file = uploaded(&deployment, &storage, &alice, "before.txt", b"x").await;

            let updated = update(&deployment, &alice, file.id, " after.txt ")
                .await
                .expect("renamed");
            assert_eq!(updated.name, "after.txt");
            assert_eq!(updated.version, 2);
            assert_eq!(updated.size, file.size);
            let same = update(&deployment, &alice, file.id, "after.txt")
                .await
                .expect("unchanged");
            assert_eq!(same.version, 2);
            let error = update(&deployment, &alice, file.id, " ")
                .await
                .expect_err("blank");
            assert!(matches!(error, FileError::Invalid(_)));

            let event = Event::update(
                TABLE,
                file.id,
                2,
                fields(json!({ "size": 1 })),
                fields(json!({ "size": 2 })),
                now(),
            );
            let answer = apply_event(&deployment, &alice, &event)
                .await
                .expect("answered");
            assert_eq!(answer.status, AnswerStatus::Rejected);
            assert_eq!(answer.reason.as_deref(), Some("not_editable: size"));
        })
        .await;
    }

    #[tokio::test]
    async fn files_delete_hides_the_file_and_records_who() {
        with_each_database(|database| async move {
            let (deployment, storage) = server(database);
            let alice = users::create(&deployment, "alice@example.com")
                .await
                .expect("a user");
            let file = uploaded(&deployment, &storage, &alice, "going.txt", b"x").await;

            delete(&deployment, &alice, file.id).await.expect("deleted");
            let error = get(&deployment, &alice, file.id).await.expect_err("gone");
            assert!(matches!(error, FileError::NotFound { .. }));
            assert!(
                list(&deployment, &alice)
                    .await
                    .expect("the files")
                    .is_empty()
            );
            let mut tx = deployment.begin().await.expect("a transaction");
            let row = get_in(&mut tx, file.id)
                .await
                .expect("the row")
                .expect("kept");
            assert!(row.deleted_at.is_some());
            assert_eq!(row.deleted_by_id, Some(alice.id));
            assert_eq!(
                storage
                    .size(&storage_key(alice.id, file.id))
                    .await
                    .expect("looked"),
                Some(1),
                "the bytes stay for the sweep"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn files_create_upload_signs_a_url_until_the_bytes_are_there() {
        with_each_database(|database| async move {
            let (deployment, storage) = server(database);
            let alice = users::create(&deployment, "alice@example.com")
                .await
                .expect("a user");
            let request = FileUploadNew {
                id: None,
                content_type: "IMAGE/GIF".to_string(),
                size: 3,
            };

            let upload = create_upload(&deployment, &alice, &request)
                .await
                .expect("where to upload");
            assert!(!upload.uploaded);
            assert_eq!(upload.content_type, "image/gif");
            let key = storage_key(alice.id, upload.id);
            assert_eq!(upload.upload_url, Some(format!("memory:///{key}")));

            storage
                .put(&key, b"GIF".to_vec())
                .await
                .expect("the upload");
            let again = FileUploadNew {
                id: Some(upload.id),
                ..request.clone()
            };
            let upload = create_upload(&deployment, &alice, &again)
                .await
                .expect("already there");
            assert!(upload.uploaded);
            assert_eq!(upload.upload_url, None);

            let too_large = FileUploadNew {
                size: MAX_BYTES + 1,
                ..request
            };
            let error = create_upload(&deployment, &alice, &too_large)
                .await
                .expect_err("too large");
            assert!(matches!(error, FileError::Invalid(_)));
        })
        .await;
    }

    #[tokio::test]
    async fn files_content_url_is_signed_for_the_owner() {
        with_each_database(|database| async move {
            let (deployment, storage) = server(database);
            let alice = users::create(&deployment, "alice@example.com")
                .await
                .expect("a user");
            let file = uploaded(&deployment, &storage, &alice, "notes.txt", b"x").await;
            let url = content_url(&deployment, &alice, file.id, false)
                .await
                .expect("a url");
            assert_eq!(url, format!("memory:///{}", storage_key(alice.id, file.id)));
            assert_eq!(
                attachment_named("my notes \"v2\".txt"),
                "attachment; filename*=UTF-8''my%20notes%20%22v2%22.txt"
            );
        })
        .await;
    }

    #[tokio::test]
    async fn files_without_a_bucket_the_server_refuses_to_create() {
        let database = Database::open_sqlite_in_memory().await.expect("a database");
        let deployment = Deployment::new(database, Role::Server);
        let alice = users::create(&deployment, "alice@example.com")
            .await
            .expect("a user");
        let error = create(&deployment, &alice, &new_file("notes.txt", b"x"))
            .await
            .expect_err("no bucket");
        assert!(matches!(error, FileError::NoFileStore));
    }
}
