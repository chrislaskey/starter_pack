//! The files commands: `app_core::files` as the pages call it with
//! `invoke`, for the user signed in on this device, see
//! `docs/design/offline-files-decisions.md`. A form's error is
//! `{error, errors}`, the web's 422 body. A write starts a sync in the
//! background once it is on the device, see `sync`.
//!
//! A page reads a file's bytes through the `files` protocol,
//! `content_response`, which serves the copy in the account's folder,
//! so an `<img>` shows it with the network off.

use percent_encoding::percent_decode_str;
use tauri::http::header::{CONTENT_TYPE, HeaderValue, X_CONTENT_TYPE_OPTIONS};
use tauri::http::{Response, StatusCode, Uri};
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

use app_core::files::{self, File, FileError, FileWithState};

use crate::avatars::FormError;
use crate::device_accounts::DeviceAccounts;
use crate::sync;
use crate::users::signed_in;

/// How many names a download tries before giving up.
const FREE_NAME_TRIES: u32 = 1000;
const FILE_NAME_HEADER: &str = "x-file-name";
const FILE_CONTENT_TYPE_HEADER: &str = "x-file-content-type";

impl From<FileError> for FormError {
    fn from(error: FileError) -> FormError {
        match error {
            FileError::Invalid(errors) => FormError::of_fields(errors),
            other => FormError::from(other.to_string()),
        }
    }
}

#[tauri::command]
pub async fn files_list(device: State<'_, DeviceAccounts>) -> Result<Vec<FileWithState>, String> {
    let (deployment, user) = signed_in(&device).await?;
    files::list(&deployment, &user)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn files_get(
    device: State<'_, DeviceAccounts>,
    id: Uuid,
) -> Result<FileWithState, String> {
    let (deployment, user) = signed_in(&device).await?;
    files::get(&deployment, &user, id)
        .await
        .map_err(|error| error.to_string())
}

/// Adds the file the person picked: its bytes are the command's raw
/// body, its name, percent-encoded, and its content type are headers.
/// The file is kept on this device and sent by the next sync.
#[tauri::command]
pub async fn files_create(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    request: Request<'_>,
) -> Result<File, FormError> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err(FormError::from(
            "the file's bytes are the command's body".to_string(),
        ));
    };
    let header = |name: &str| {
        request
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
    };
    let name = percent_decode_str(header(FILE_NAME_HEADER)).decode_utf8_lossy();
    let (deployment, user) = signed_in(&device).await?;
    let file = files::create_with_bytes(
        &deployment,
        &user,
        &name,
        header(FILE_CONTENT_TYPE_HEADER),
        bytes,
    )
    .await?;
    sync::spawn(app);
    Ok(file)
}

#[tauri::command]
pub async fn files_update(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    id: Uuid,
    name: String,
) -> Result<File, FormError> {
    let (deployment, user) = signed_in(&device).await?;
    let file = files::update(&deployment, &user, id, &name).await?;
    sync::spawn(app);
    Ok(file)
}

#[tauri::command]
pub async fn files_delete(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    id: Uuid,
) -> Result<(), String> {
    let (deployment, user) = signed_in(&device).await?;
    files::delete(&deployment, &user, id)
        .await
        .map_err(|error| error.to_string())?;
    sync::spawn(app);
    Ok(())
}

/// Whether this build saves a copy of a file outside the app: the
/// desktop does, into the Downloads folder; a phone does not yet.
#[tauri::command]
pub fn files_download_supported() -> bool {
    cfg!(desktop)
}

/// Copies the file from this device into the Downloads folder under its
/// name, or the name with a number when that is taken; answers where.
#[tauri::command]
pub async fn files_download(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    id: Uuid,
) -> Result<String, String> {
    if !cfg!(desktop) {
        return Err("Downloading is not available on this device yet".to_string());
    }
    let (deployment, user) = signed_in(&device).await?;
    let shown = files::get(&deployment, &user, id)
        .await
        .map_err(|error| error.to_string())?;
    let from = files::local_path(&deployment, id).map_err(|error| error.to_string())?;
    let downloads = app
        .path()
        .download_dir()
        .map_err(|error| error.to_string())?;
    let to = tokio::task::spawn_blocking(move || {
        copy_under_a_free_name(&from, &downloads, &shown.file.name)
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| format!("could not save the file: {error}"))?;
    Ok(to.display().to_string())
}

/// Copies the file into the folder under the name, without replacing
/// anything there: `notes.txt`, then `notes (1).txt`, and so on.
fn copy_under_a_free_name(
    from: &std::path::Path,
    folder: &std::path::Path,
    name: &str,
) -> std::io::Result<std::path::PathBuf> {
    // A name is the person's text: only its last part, so it stays in the folder
    let name = std::path::Path::new(name)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let (stem, extension) = match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => (stem, format!(".{extension}")),
        _ => (name, String::new()),
    };
    let mut bytes = std::fs::File::open(from)?;
    for number in 0..FREE_NAME_TRIES {
        let candidate = match number {
            0 => folder.join(name),
            number => folder.join(format!("{stem} ({number}){extension}")),
        };
        match std::fs::File::create_new(&candidate) {
            Ok(mut copy) => {
                std::io::copy(&mut bytes, &mut copy)?;
                return Ok(candidate);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        format!("{name} and its numbered copies are all taken"),
    ))
}

/// Answers a request of the `files` protocol, `files://localhost/<id>`:
/// the bytes of the signed-in user's file from this device, with the
/// row's content type, or 404. `nosniff`, so the webview shows a file as
/// the type its row says and never as a page of its own guessing.
pub async fn content_response(app: &AppHandle, uri: &Uri) -> Response<Vec<u8>> {
    match content(app, uri).await {
        Some((content_type, bytes)) => {
            let mut response = Response::new(bytes);
            let content_type = HeaderValue::from_str(&content_type)
                .unwrap_or(HeaderValue::from_static(files::UNKNOWN_CONTENT_TYPE));
            response.headers_mut().insert(CONTENT_TYPE, content_type);
            response
                .headers_mut()
                .insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
            response
        }
        None => {
            let mut response = Response::new(Vec::new());
            *response.status_mut() = StatusCode::NOT_FOUND;
            response
        }
    }
}

/// The content type and the bytes of the file the URI names, when the
/// signed-in user has it on this device.
async fn content(app: &AppHandle, uri: &Uri) -> Option<(String, Vec<u8>)> {
    let id = Uuid::parse_str(uri.path().trim_start_matches('/')).ok()?;
    let device = app.state::<DeviceAccounts>();
    let (deployment, user) = signed_in(&device).await.ok()?;
    let shown = files::get(&deployment, &user, id).await.ok()?;
    let path = files::local_path(&deployment, id).ok()?;
    let bytes = tokio::fs::read(path).await.ok()?;
    Some((shown.file.content_type, bytes))
}
