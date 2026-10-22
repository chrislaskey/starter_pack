//! The API token the app connects to the websocket API with.
//!
//! The token is kept in a file in the app's data directory, readable only by
//! the user, so the app reconnects on the next launch without logging in
//! again. The Phoenix server only stores a hash of it.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct TokenStore {
    path: PathBuf,
    token: Mutex<Option<String>>,
}

impl TokenStore {
    pub fn open(data_dir: PathBuf) -> Self {
        let path = data_dir.join("api_token");
        let token = match fs::read_to_string(&path) {
            Ok(contents) if !contents.trim().is_empty() => Some(contents.trim().to_string()),
            Ok(_) => None,
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => {
                log::warn!(
                    "Could not read the API token at {}: {error}",
                    path.display()
                );
                None
            }
        };

        Self {
            path,
            token: Mutex::new(token),
        }
    }

    pub fn get(&self) -> Option<String> {
        self.token.lock().unwrap().clone()
    }

    pub fn set(&self, token: &str) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(|error| error.to_string())?;
        }
        fs::write(&self.path, token).map_err(|error| error.to_string())?;
        restrict_permissions(&self.path)?;

        *self.token.lock().unwrap() = Some(token.to_string());
        Ok(())
    }

    pub fn clear(&self) -> Result<(), String> {
        match fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }

        *self.token.lock().unwrap() = None;
        Ok(())
    }
}

#[cfg(unix)]
fn restrict_permissions(path: &std::path::Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &std::path::Path) -> Result<(), String> {
    Ok(())
}
