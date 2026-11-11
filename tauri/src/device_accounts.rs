//! The accounts on this device, see `docs/design/device-accounts.md`:
//! each with a folder of its own, `accounts/<user id>/`, holding its
//! SQLite database, `app.db`, and the bytes of its files, `files/`, see
//! `docs/design/offline-files-decisions.md`; listed in `device.db`, and
//! at most one active, whose deployment every account command takes
//! from here. This is the only code that opens, closes, chooses or
//! deletes an account's folder; `app_core` never learns there is more
//! than one. The app data directory, `DATA_DIR` in dev, holds them all.
//! At start the list is swept of orphans, every database is brought up
//! to date, and the last active account is opened again if it is still
//! signed in.

use std::env;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use tauri::{App, AppHandle, Manager, State};
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;
use uuid::fmt::Hyphenated;

use app_core::accounts::{self, AccountError, Session};
use app_core::database::{Database, DatabaseError};
use app_core::deployment::{Deployment, FileStore, Role, now};
use app_core::device_settings::{self, DeviceSettingsError};
use app_core::sync::{self, SyncError, Transport};
use app_core::users::{self, User, UserError};

use crate::sync::SyncServer;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations/device");

/// How many accounts a device holds at most.
pub const MAX_ACCOUNTS: i64 = 100;
const LIST_FILE: &str = "device.db";
const ACCOUNTS_DIRECTORY: &str = "accounts";
const DATABASE_FILE: &str = "app.db";
const FILES_DIRECTORY: &str = "files";

/// An account on this device, as the accounts page lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DeviceAccount {
    pub user_id: Uuid,
    pub email: String,
    /// The account's file holds a token: it switches to at once;
    /// otherwise it asks for the password first.
    pub signed_in: bool,
    pub active: bool,
    pub last_active_at: DateTime<Utc>,
}

/// What a removal answers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Removal {
    Removed,
    /// Nothing was done: these changes have not reached the server, or
    /// were not kept by it, and the removal would lose them. The page
    /// asks the person, who removes `losing_changes` or keeps the account.
    ChangesWouldBeLost {
        pending: i64,
        conflicts: i64,
    },
}

#[derive(Debug, Error)]
pub enum DeviceAccountsError {
    #[error("Nobody is signed in")]
    NobodyActive,
    #[error("No account {user_id} on this device")]
    UnknownAccount { user_id: Uuid },
    #[error("{email} is signed out on this device; sign in first")]
    NotSignedIn { email: String },
    #[error("This device holds {MAX_ACCOUNTS} accounts; remove one first")]
    TooManyAccounts,
    #[error(transparent)]
    Account(#[from] AccountError),
    #[error(transparent)]
    User(#[from] UserError),
    #[error(transparent)]
    Sync(#[from] SyncError),
    #[error(transparent)]
    DeviceSettings(#[from] DeviceSettingsError),
    #[error("could not open the account's database: {0}")]
    Open(#[from] DatabaseError),
    #[error("device.db could not be brought up to date: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
}

/// The list in `device.db`, the folders under `accounts/`, and the
/// active account's open deployment, if any.
pub struct DeviceAccounts {
    directory: PathBuf,
    list: SqlitePool,
    active: RwLock<Option<Active>>,
}

struct Active {
    user_id: Uuid,
    deployment: Deployment,
}

#[derive(sqlx::FromRow)]
struct Row {
    user_id: Hyphenated,
    email: String,
    file: String,
    signed_in: bool,
    active: bool,
    last_active_at: DateTime<Utc>,
}

impl From<Row> for DeviceAccount {
    fn from(row: Row) -> DeviceAccount {
        DeviceAccount {
            user_id: row.user_id.into_uuid(),
            email: row.email,
            signed_in: row.signed_in,
            active: row.active,
            last_active_at: row.last_active_at,
        }
    }
}

/// Opens the device's accounts as the app starts and hands them to the
/// commands as managed state.
pub fn open_and_manage(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let directory = match env::var_os("DATA_DIR") {
        Some(directory) => PathBuf::from(directory),
        None => app.path().app_data_dir()?,
    };
    let accounts = tauri::async_runtime::block_on(DeviceAccounts::open(&directory))?;
    log::info!("accounts in {}", directory.display());
    app.manage(accounts);
    Ok(())
}

impl DeviceAccounts {
    /// Opens `device.db` in the directory, creating both when missing,
    /// sweeps the orphans, brings every account's file up to date and
    /// opens the last active account again if it is signed in.
    pub async fn open(directory: &Path) -> Result<DeviceAccounts, DeviceAccountsError> {
        std::fs::create_dir_all(directory.join(ACCOUNTS_DIRECTORY))?;
        let options = SqliteConnectOptions::new()
            .filename(directory.join(LIST_FILE))
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        let list = SqlitePoolOptions::new().connect_with(options).await?;
        MIGRATOR.run(&list).await?;
        let accounts = DeviceAccounts {
            directory: directory.to_path_buf(),
            list,
            active: RwLock::new(None),
        };
        accounts.sweep().await?;
        accounts.migrate_every_file().await?;
        accounts.open_last_active().await?;
        Ok(accounts)
    }

    /// Every account, the most recently active first.
    pub async fn list(&self) -> Result<Vec<DeviceAccount>, DeviceAccountsError> {
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT user_id, email, file, signed_in, active, last_active_at
             FROM device_accounts ORDER BY last_active_at DESC, email",
        )
        .fetch_all(&self.list)
        .await?;
        Ok(rows.into_iter().map(DeviceAccount::from).collect())
    }

    /// The active account's deployment, for the account commands.
    pub async fn active(&self) -> Result<Deployment, DeviceAccountsError> {
        self.active
            .read()
            .await
            .as_ref()
            .map(|active| active.deployment.clone())
            .ok_or(DeviceAccountsError::NobodyActive)
    }

    pub async fn active_user_id(&self) -> Option<Uuid> {
        self.active
            .read()
            .await
            .as_ref()
            .map(|active| active.user_id)
    }

    /// Keeps a sign-in the server answered: in the account's own file,
    /// made for it when new, which becomes the active one. Whoever was
    /// active before is closed first, and stays signed in.
    pub async fn keep_signed_in(&self, session: Session) -> Result<User, DeviceAccountsError> {
        self.deactivate().await?;
        let user_id = session.user.id;
        if self.get(user_id).await?.is_none() && self.count().await? >= MAX_ACCOUNTS {
            return Err(DeviceAccountsError::TooManyAccounts);
        }
        // The row first, so a crash before the file leaves a row the
        // sweep removes, never a file nothing points at.
        sqlx::query(
            "INSERT INTO device_accounts (user_id, email, file, signed_in, last_active_at, active)
             VALUES ($1, $2, $3, 0, $4, 0)
             ON CONFLICT (user_id) DO UPDATE SET email = excluded.email",
        )
        .bind(user_id.hyphenated())
        .bind(&session.email)
        .bind(file_name(user_id))
        .bind(now())
        .execute(&self.list)
        .await?;
        let deployment = self.open_file(user_id).await?;
        let user = accounts::keep_signed_in(&deployment, session).await?;
        self.mark(user_id, true, true).await?;
        *self.active.write().await = Some(Active {
            user_id,
            deployment,
        });
        Ok(user)
    }

    /// Makes a signed-in account the active one, closing whoever was.
    pub async fn activate(&self, user_id: Uuid) -> Result<User, DeviceAccountsError> {
        let account = self
            .get(user_id)
            .await?
            .ok_or(DeviceAccountsError::UnknownAccount { user_id })?;
        if !account.signed_in {
            return Err(DeviceAccountsError::NotSignedIn {
                email: account.email,
            });
        }
        self.deactivate().await?;
        let deployment = self.open_file(user_id).await?;
        let user = users::signed_in(&deployment).await?;
        self.mark(user_id, true, true).await?;
        *self.active.write().await = Some(Active {
            user_id,
            deployment,
        });
        Ok(user)
    }

    /// Closes the active account, if any, which stays signed in. The
    /// commands hold the sync lock around this, so no sync is mid-way.
    pub async fn deactivate(&self) -> Result<(), DeviceAccountsError> {
        if let Some(active) = self.active.write().await.take() {
            sqlx::query("UPDATE device_accounts SET active = 0 WHERE user_id = $1")
                .bind(active.user_id.hyphenated())
                .execute(&self.list)
                .await?;
            active.deployment.database.close().await;
        }
        Ok(())
    }

    /// Signs the active account out: the server forgets the token, the
    /// file keeps its data, and nobody is active.
    pub async fn sign_out<T: Transport>(&self, transport: &T) -> Result<(), DeviceAccountsError> {
        let (user_id, deployment) = self.active_pair().await?;
        accounts::sign_out_on_device(&deployment, transport).await?;
        self.mark(user_id, false, false).await?;
        self.deactivate().await
    }

    /// Signs the active account out without telling the server, which
    /// answered 401: it has forgotten the token already. The data stays.
    pub async fn forget_signed_in(&self) -> Result<(), DeviceAccountsError> {
        let (user_id, deployment) = self.active_pair().await?;
        accounts::forget_signed_in(&deployment).await?;
        self.mark(user_id, false, false).await?;
        self.deactivate().await
    }

    /// Removes an account from the device: signs it out when it is
    /// signed in, then deletes its folder, the database and the files'
    /// bytes, and its row. Answers the counts
    /// instead while a change would be lost, unless `losing_changes`.
    pub async fn remove<T: Transport>(
        &self,
        transport: &T,
        user_id: Uuid,
        losing_changes: bool,
    ) -> Result<Removal, DeviceAccountsError> {
        let account = self
            .get(user_id)
            .await?
            .ok_or(DeviceAccountsError::UnknownAccount { user_id })?;
        let was_active = self.active_user_id().await == Some(user_id);
        let deployment = if was_active {
            self.active().await?
        } else {
            self.open_file(user_id).await?
        };
        let outcome = self
            .remove_opened(transport, &account, &deployment, losing_changes)
            .await;
        let removed = outcome
            .as_ref()
            .is_ok_and(|outcome| *outcome == Removal::Removed);
        if !was_active {
            deployment.database.close().await;
        } else if removed {
            self.deactivate().await?;
        }
        if removed {
            sqlx::query("DELETE FROM device_accounts WHERE user_id = $1")
                .bind(user_id.hyphenated())
                .execute(&self.list)
                .await?;
            delete_folder(&self.folder_of(user_id))?;
        }
        outcome
    }

    async fn remove_opened<T: Transport>(
        &self,
        transport: &T,
        account: &DeviceAccount,
        deployment: &Deployment,
        losing_changes: bool,
    ) -> Result<Removal, DeviceAccountsError> {
        if !losing_changes {
            let status = sync::status(deployment).await?;
            if status.pending > 0 || status.conflicts > 0 {
                return Ok(Removal::ChangesWouldBeLost {
                    pending: status.pending,
                    conflicts: status.conflicts,
                });
            }
        }
        if account.signed_in {
            match accounts::sign_out_on_device(deployment, transport).await {
                Ok(()) | Err(AccountError::User(UserError::NotSignedIn)) => {}
                Err(error) => return Err(error.into()),
            }
            self.mark(account.user_id, false, false).await?;
        }
        Ok(Removal::Removed)
    }

    async fn active_pair(&self) -> Result<(Uuid, Deployment), DeviceAccountsError> {
        self.active
            .read()
            .await
            .as_ref()
            .map(|active| (active.user_id, active.deployment.clone()))
            .ok_or(DeviceAccountsError::NobodyActive)
    }

    async fn get(&self, user_id: Uuid) -> Result<Option<DeviceAccount>, DeviceAccountsError> {
        let row: Option<Row> = sqlx::query_as(
            "SELECT user_id, email, file, signed_in, active, last_active_at
             FROM device_accounts WHERE user_id = $1",
        )
        .bind(user_id.hyphenated())
        .fetch_optional(&self.list)
        .await?;
        Ok(row.map(DeviceAccount::from))
    }

    async fn count(&self) -> Result<i64, DeviceAccountsError> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM device_accounts")
            .fetch_one(&self.list)
            .await?;
        Ok(count)
    }

    async fn mark(
        &self,
        user_id: Uuid,
        signed_in: bool,
        active: bool,
    ) -> Result<(), DeviceAccountsError> {
        sqlx::query(
            "UPDATE device_accounts SET signed_in = $2, active = $3, last_active_at = $4
             WHERE user_id = $1",
        )
        .bind(user_id.hyphenated())
        .bind(signed_in)
        .bind(active)
        .bind(now())
        .execute(&self.list)
        .await?;
        Ok(())
    }

    /// The account's folder: its database and its files' bytes.
    fn folder_of(&self, user_id: Uuid) -> PathBuf {
        self.directory
            .join(ACCOUNTS_DIRECTORY)
            .join(user_id.hyphenated().to_string())
    }

    fn path_of(&self, user_id: Uuid) -> PathBuf {
        self.folder_of(user_id).join(DATABASE_FILE)
    }

    /// Opens an account's database as a device, bringing it and its
    /// settings up to date, with the folder its files' bytes are in.
    async fn open_file(&self, user_id: Uuid) -> Result<Deployment, DeviceAccountsError> {
        let files = self.folder_of(user_id).join(FILES_DIRECTORY);
        std::fs::create_dir_all(&files)?;
        let database = Database::open_sqlite_file(&self.path_of(user_id)).await?;
        let deployment =
            Deployment::new(database, Role::Device).with_file_store(FileStore::Folder(files));
        device_settings::upgrade(&deployment).await?;
        Ok(deployment)
    }

    /// Deletes a row whose database is missing, a half-finished sign-in,
    /// and whatever under `accounts/` no row names, a half-finished
    /// removal.
    async fn sweep(&self) -> Result<(), DeviceAccountsError> {
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT user_id, email, file, signed_in, active, last_active_at FROM device_accounts",
        )
        .fetch_all(&self.list)
        .await?;
        let accounts_directory = self.directory.join(ACCOUNTS_DIRECTORY);
        let mut named = Vec::new();
        for row in rows {
            let user_id = row.user_id.into_uuid();
            if row.file == file_name(user_id) && accounts_directory.join(&row.file).exists() {
                named.push(user_id.hyphenated().to_string());
            } else {
                log::warn!("account {} has no file; removed from the list", row.email);
                sqlx::query("DELETE FROM device_accounts WHERE user_id = $1")
                    .bind(user_id.hyphenated())
                    .execute(&self.list)
                    .await?;
            }
        }
        for entry in std::fs::read_dir(&accounts_directory)? {
            let path = entry?.path();
            let name = path.file_name().and_then(|name| name.to_str());
            if name.is_some_and(|name| !named.iter().any(|folder| folder == name)) {
                log::warn!("{} is no account's; deleted", path.display());
                delete_folder(&path)?;
            }
        }
        Ok(())
    }

    /// Opens and closes every account's file, so a migration that fails
    /// does so now, on the update, and not on a switch weeks later.
    async fn migrate_every_file(&self) -> Result<(), DeviceAccountsError> {
        for account in self.list().await? {
            let deployment = self.open_file(account.user_id).await?;
            deployment.database.close().await;
        }
        Ok(())
    }

    async fn open_last_active(&self) -> Result<(), DeviceAccountsError> {
        let last = self
            .list()
            .await?
            .into_iter()
            .find(|account| account.active);
        if let Some(account) = last {
            if account.signed_in {
                self.activate(account.user_id).await?;
            } else {
                self.mark(account.user_id, false, false).await?;
            }
        }
        Ok(())
    }
}

/// The account's database, as its row names it: under `accounts/`.
fn file_name(user_id: Uuid) -> String {
    format!("{}/{DATABASE_FILE}", user_id.hyphenated())
}

/// Deletes an account's folder with everything in it, or a stray file.
fn delete_folder(path: &Path) -> std::io::Result<()> {
    let deleted = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    match deleted {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

#[tauri::command]
pub async fn device_accounts_list(
    device: State<'_, DeviceAccounts>,
) -> Result<Vec<DeviceAccount>, String> {
    device.list().await.map_err(|error| error.to_string())
}

/// Switches to a signed-in account, which then syncs.
#[tauri::command]
pub async fn device_accounts_switch(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    user_id: Uuid,
) -> Result<User, String> {
    let user = {
        let _no_sync_meanwhile = server.hold().await;
        device
            .activate(user_id)
            .await
            .map_err(|error| error.to_string())?
    };
    crate::sync::spawn(app);
    Ok(user)
}

/// Closes the active account, before adding another: nobody is active
/// while the sign-in page shows.
#[tauri::command]
pub async fn device_accounts_deactivate(
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
) -> Result<(), String> {
    let _no_sync_meanwhile = server.hold().await;
    device.deactivate().await.map_err(|error| error.to_string())
}

/// Removes an account from the device, syncing the active one first so
/// a change still waiting reaches the server rather than being lost.
#[tauri::command]
pub async fn device_accounts_remove(
    app: AppHandle,
    device: State<'_, DeviceAccounts>,
    server: State<'_, SyncServer>,
    user_id: Uuid,
    losing_changes: bool,
) -> Result<Removal, String> {
    if device.active_user_id().await == Some(user_id) {
        // Reported by the sync itself; what it could not push is
        // answered as changes that would be lost.
        let _ = crate::sync::run(&app).await;
    }
    let _no_sync_meanwhile = server.hold().await;
    device
        .remove(server.transport(), user_id, losing_changes)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use app_core::mailer::Mailer;
    use app_core::sync::InProcess;
    use app_core::{mails, posts};

    async fn server() -> Deployment {
        let database = Database::open_sqlite_in_memory()
            .await
            .expect("an in-memory database");
        Deployment::new(database, Role::Server)
    }

    /// A user signed up on the server and finished with the code the
    /// memory mailer caught, as a device: the session its exchange
    /// answered. Once the email has an account, a sign-in instead.
    async fn signed_up(server: &Deployment, email: &str) -> Session {
        let mailer = Mailer::memory();
        let sent = accounts::sign_up(server, &mailer, email, "Alice")
            .await
            .expect("signed up");
        let mail = mailer.last_sent_to(email).expect("mailed");
        let sent = match mails::code_in(&mail) {
            Some(_) => sent,
            None => accounts::sign_in(server, &mailer, email)
                .await
                .expect("signed in"),
        };
        let code = mails::code_in(&mailer.last_sent_to(email).expect("mailed")).expect("a code");
        let (verifier, challenge) = accounts::verifier_and_challenge();
        let step = accounts::sign_in_with_code_for_device(server, &sent.pending, &code, &challenge)
            .await
            .expect("the code matched");
        let device_code = match step {
            accounts::SignInStep::Finished(device_code) => device_code,
            accounts::SignInStep::Next { pending, .. } => {
                accounts::sign_in_with_new_password_for_device(server, &pending, None, &challenge)
                    .await
                    .expect("skipped making a password")
            }
        };
        accounts::sign_in_with_device_code(server, &device_code.code, &verifier)
            .await
            .expect("exchanged")
    }

    fn file_exists(device: &DeviceAccounts, user_id: Uuid) -> bool {
        device.path_of(user_id).exists()
    }

    #[tokio::test]
    async fn device_accounts_sign_in_switch_sign_out_and_remove() {
        let directory = tempfile::tempdir().expect("a directory");
        let server = server().await;
        let transport = InProcess { server: &server };
        let device = DeviceAccounts::open(directory.path())
            .await
            .expect("opened");
        assert!(matches!(
            device.active().await,
            Err(DeviceAccountsError::NobodyActive)
        ));

        let alice = device
            .keep_signed_in(signed_up(&server, "alice@example.com").await)
            .await
            .expect("alice kept");
        let post = posts::create(
            &device.active().await.expect("alice's"),
            &alice,
            "hers",
            &[],
        )
        .await
        .expect("a post");
        sync::run(&device.active().await.expect("alice's"), &transport)
            .await
            .expect("synced");

        let bob = device
            .keep_signed_in(signed_up(&server, "bob@example.com").await)
            .await
            .expect("bob kept, alice closed");
        assert_eq!(device.active_user_id().await, Some(bob.id));
        let listed = device.list().await.expect("listed");
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().all(|account| account.signed_in));
        assert_eq!(
            listed
                .iter()
                .find(|account| account.active)
                .map(|a| a.user_id),
            Some(bob.id)
        );

        let again = device.activate(alice.id).await.expect("alice again");
        assert_eq!(again, alice);
        let hers = posts::list(&device.active().await.expect("alice's"), &alice)
            .await
            .expect("her posts");
        assert_eq!(hers.len(), 1, "her file was kept while bob was active");
        assert_eq!(hers[0].post.id, post.id);

        device.sign_out(&transport).await.expect("signed out");
        assert_eq!(device.active_user_id().await, None);
        let error = device.activate(alice.id).await.expect_err("signed out");
        assert!(matches!(error, DeviceAccountsError::NotSignedIn { .. }));
        assert!(file_exists(&device, alice.id), "her data stays");

        let removed = device
            .remove(&transport, bob.id, false)
            .await
            .expect("removed");
        assert_eq!(removed, Removal::Removed);
        assert!(!file_exists(&device, bob.id));
        assert!(
            !device.folder_of(bob.id).exists(),
            "his folder went, the files' bytes with it"
        );
        assert!(device.folder_of(alice.id).join(FILES_DIRECTORY).is_dir());
        assert_eq!(device.list().await.expect("listed").len(), 1);
        let error = device
            .remove(&transport, bob.id, false)
            .await
            .expect_err("gone");
        assert!(matches!(error, DeviceAccountsError::UnknownAccount { .. }));

        let removed = device
            .remove(&transport, alice.id, false)
            .await
            .expect("a signed-out account removes too");
        assert_eq!(removed, Removal::Removed);
        assert!(device.list().await.expect("listed").is_empty());
    }

    #[tokio::test]
    async fn device_accounts_remove_asks_before_losing_changes() {
        let directory = tempfile::tempdir().expect("a directory");
        let server = server().await;
        let transport = InProcess { server: &server };
        let device = DeviceAccounts::open(directory.path())
            .await
            .expect("opened");
        let alice = device
            .keep_signed_in(signed_up(&server, "alice@example.com").await)
            .await
            .expect("alice kept");
        let post = posts::create(
            &device.active().await.expect("alice's"),
            &alice,
            "never synced",
            &[],
        )
        .await
        .expect("a post");

        let asked = device
            .remove(&transport, alice.id, false)
            .await
            .expect("asked");
        assert_eq!(
            asked,
            Removal::ChangesWouldBeLost {
                pending: 1,
                conflicts: 0
            }
        );
        assert_eq!(
            device.active_user_id().await,
            Some(alice.id),
            "nothing done"
        );

        let removed = device
            .remove(&transport, alice.id, true)
            .await
            .expect("removed");
        assert_eq!(removed, Removal::Removed);
        assert_eq!(device.active_user_id().await, None);
        assert!(!file_exists(&device, alice.id));
        assert!(
            posts::get(&server, &alice, post.id).await.is_err(),
            "the change never reached the server"
        );
    }

    #[tokio::test]
    async fn device_accounts_open_sweeps_orphans_and_opens_the_last_active_again() {
        let directory = tempfile::tempdir().expect("a directory");
        let server = server().await;
        let alice_id = {
            let device = DeviceAccounts::open(directory.path())
                .await
                .expect("opened");
            let alice = device
                .keep_signed_in(signed_up(&server, "alice@example.com").await)
                .await
                .expect("alice kept");
            device
                .keep_signed_in(signed_up(&server, "bob@example.com").await)
                .await
                .expect("bob kept");
            device.activate(alice.id).await.expect("alice active");
            // A row without a database, and a folder without a row
            sqlx::query(
                "INSERT INTO device_accounts (user_id, email, file, signed_in, last_active_at)
                 VALUES ($1, 'carol@example.com', 'carol.db', 1, $2)",
            )
            .bind(Uuid::now_v7().hyphenated())
            .bind(now())
            .execute(&device.list)
            .await
            .expect("a stray row");
            let stray = directory.path().join("accounts/stray/files");
            std::fs::create_dir_all(&stray).expect("a stray folder");
            std::fs::write(stray.join("a-file"), b"bytes").expect("a stray file");
            device.deactivate().await.expect("closed");
            device.activate(alice.id).await.expect("alice active again");
            alice.id
        };

        let device = DeviceAccounts::open(directory.path())
            .await
            .expect("opened again");
        assert_eq!(device.active_user_id().await, Some(alice_id));
        let listed = device.list().await.expect("listed");
        assert_eq!(listed.len(), 2, "the stray row is gone");
        assert!(!directory.path().join("accounts/stray").exists());
        assert!(file_exists(&device, alice_id));
    }

    #[tokio::test]
    async fn device_accounts_a_401_is_a_sign_out_done_elsewhere() {
        let directory = tempfile::tempdir().expect("a directory");
        let server = server().await;
        let transport = InProcess { server: &server };
        let device = DeviceAccounts::open(directory.path())
            .await
            .expect("opened");
        let session = signed_up(&server, "alice@example.com").await;
        let alice = device
            .keep_signed_in(session.clone())
            .await
            .expect("alice kept");
        posts::create(
            &device.active().await.expect("alice's"),
            &alice,
            "kept through it",
            &[],
        )
        .await
        .expect("a post");
        accounts::sign_out(&server, &session.token)
            .await
            .expect("the token deleted elsewhere");

        let error = sync::run(&device.active().await.expect("alice's"), &transport)
            .await
            .expect_err("401");
        assert!(matches!(error, SyncError::Unauthorized));
        device.forget_signed_in().await.expect("forgotten");
        assert_eq!(device.active_user_id().await, None);
        let listed = device.list().await.expect("listed");
        assert!(!listed[0].signed_in);

        let again = device
            .keep_signed_in(signed_up(&server, "alice@example.com").await)
            .await
            .expect("alice kept again");
        assert_eq!(again, alice);
        let status = sync::status(&device.active().await.expect("alice's"))
            .await
            .expect("status");
        assert_eq!(status.pending, 1, "her change waited through it");
    }

    #[tokio::test]
    async fn device_accounts_hold_at_most_the_cap() {
        let directory = tempfile::tempdir().expect("a directory");
        let server = server().await;
        let device = DeviceAccounts::open(directory.path())
            .await
            .expect("opened");
        for n in 0..MAX_ACCOUNTS {
            sqlx::query(
                "INSERT INTO device_accounts (user_id, email, file, signed_in, last_active_at)
                 VALUES ($1, $2, $3, 0, $4)",
            )
            .bind(Uuid::now_v7().hyphenated())
            .bind(format!("user{n}@example.com"))
            .bind(format!("user{n}.db"))
            .bind(now())
            .execute(&device.list)
            .await
            .expect("a row");
        }
        let error = device
            .keep_signed_in(signed_up(&server, "one-too-many@example.com").await)
            .await
            .expect_err("full");
        assert!(matches!(error, DeviceAccountsError::TooManyAccounts));
    }
}

/// How long a sign-in started in the browser is waited for, the pending
/// sign-in's own validity on the server.
pub const SIGN_IN_WAIT_MINUTES: i64 = app_core::tokens::SIGN_IN_CODE_VALIDITY_MINUTES;

/// A sign-in waiting on the browser, as the accounts page shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PendingSignIn {
    pub email: Option<String>,
    pub started_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct SignInRow {
    verifier: String,
    email: Option<String>,
    started_at: DateTime<Utc>,
}

/// The sign-in waiting on the browser, kept in `device.db` so a phone
/// that ended the app meanwhile picks it up again, see
/// `docs/design/accounts-sign-in.md`.
impl DeviceAccounts {
    /// Keeps the verifier of a sign-in about to open in the browser, in
    /// place of any waiting one.
    pub async fn sign_in_start(
        &self,
        verifier: &str,
        email: Option<&str>,
    ) -> Result<(), DeviceAccountsError> {
        sqlx::query(
            "INSERT INTO device_sign_in (id, verifier, email, started_at) VALUES (1, $1, $2, $3)
             ON CONFLICT (id) DO UPDATE SET verifier = excluded.verifier, email = excluded.email,
             started_at = excluded.started_at",
        )
        .bind(verifier)
        .bind(email)
        .bind(now())
        .execute(&self.list)
        .await?;
        Ok(())
    }

    /// The sign-in waiting on the browser, if one started less than
    /// `SIGN_IN_WAIT_MINUTES` ago; a stale one is forgotten.
    pub async fn sign_in_pending(&self) -> Result<Option<PendingSignIn>, DeviceAccountsError> {
        Ok(self.sign_in_row().await?.map(|row| PendingSignIn {
            email: row.email,
            started_at: row.started_at,
        }))
    }

    /// The verifier of the sign-in waiting on the browser, for the
    /// exchange of its code.
    pub async fn sign_in_verifier(&self) -> Result<Option<String>, DeviceAccountsError> {
        Ok(self.sign_in_row().await?.map(|row| row.verifier))
    }

    /// Forgets the sign-in waiting on the browser: finished or cancelled.
    pub async fn sign_in_clear(&self) -> Result<(), DeviceAccountsError> {
        sqlx::query("DELETE FROM device_sign_in")
            .execute(&self.list)
            .await?;
        Ok(())
    }

    async fn sign_in_row(&self) -> Result<Option<SignInRow>, DeviceAccountsError> {
        let row: Option<SignInRow> =
            sqlx::query_as("SELECT verifier, email, started_at FROM device_sign_in WHERE id = 1")
                .fetch_optional(&self.list)
                .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        if row.started_at < now() - chrono::Duration::minutes(SIGN_IN_WAIT_MINUTES) {
            self.sign_in_clear().await?;
            return Ok(None);
        }
        Ok(Some(row))
    }
}

#[cfg(test)]
mod sign_in_tests {
    use super::*;

    #[tokio::test]
    async fn device_accounts_a_sign_in_waits_on_the_browser_until_finished_cancelled_or_stale() {
        let directory = tempfile::tempdir().expect("a directory");
        let device = DeviceAccounts::open(directory.path())
            .await
            .expect("opened");
        assert_eq!(device.sign_in_pending().await.expect("none"), None);
        assert_eq!(device.sign_in_verifier().await.expect("none"), None);

        device
            .sign_in_start("the verifier", Some("alice@example.com"))
            .await
            .expect("started");
        let pending = device
            .sign_in_pending()
            .await
            .expect("one")
            .expect("waiting");
        assert_eq!(pending.email.as_deref(), Some("alice@example.com"));
        assert_eq!(
            device.sign_in_verifier().await.expect("one").as_deref(),
            Some("the verifier")
        );
        device
            .sign_in_start("another verifier", None)
            .await
            .expect("started again, replacing it");
        assert_eq!(
            device.sign_in_verifier().await.expect("one").as_deref(),
            Some("another verifier")
        );
        device.sign_in_clear().await.expect("cancelled");
        assert_eq!(device.sign_in_pending().await.expect("none"), None);

        device.sign_in_start("stale", None).await.expect("started");
        let long_ago = now() - chrono::Duration::minutes(SIGN_IN_WAIT_MINUTES + 1);
        sqlx::query("UPDATE device_sign_in SET started_at = $1")
            .bind(long_ago)
            .execute(&device.list)
            .await
            .expect("aged");
        assert_eq!(
            device.sign_in_pending().await.expect("none"),
            None,
            "a stale one is forgotten"
        );
        assert_eq!(device.sign_in_verifier().await.expect("none"), None);
    }
}
