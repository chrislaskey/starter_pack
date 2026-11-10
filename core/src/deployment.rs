//! A deployment: the database this web server or app opened, its role,
//! and where it keeps the bytes of files, see `files`. The web server is
//! the system of record; a desktop or mobile app is a device with a copy
//! that syncs with it, see `docs/design/offline-sync.md`. Reads are the
//! same in both roles. A write differs in two places: the server
//! increments the row's version and records the event in its log, a
//! device leaves the version as the server's and queues the event for
//! the next push.

use std::path::PathBuf;

use chrono::{DateTime, SubsecRound, Utc};

use crate::database::{Database, Transaction};
#[cfg(feature = "object-storage")]
use crate::storage::Storage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// The web server, the system of record.
    Server,
    /// A desktop or mobile app, syncing its copy with the server.
    Device,
}

/// Where a deployment keeps the bytes of files: the server in its
/// bucket, a device in the folder of the account its database is.
#[derive(Clone, Debug, Default)]
pub enum FileStore {
    /// A server with no bucket, or a test that adds no file.
    #[default]
    None,
    Folder(PathBuf),
    #[cfg(feature = "object-storage")]
    Bucket(Storage),
}

#[derive(Clone, Debug)]
pub struct Deployment {
    pub database: Database,
    pub role: Role,
    pub file_store: FileStore,
}

impl Deployment {
    /// A deployment that keeps no files' bytes, until `with_file_store`.
    pub fn new(database: Database, role: Role) -> Deployment {
        Deployment {
            database,
            role,
            file_store: FileStore::None,
        }
    }

    pub fn with_file_store(self, file_store: FileStore) -> Deployment {
        Deployment { file_store, ..self }
    }

    /// Begins the one transaction a core function runs in.
    pub async fn begin(&self) -> Result<Transaction, sqlx::Error> {
        self.database.begin().await
    }
}

/// The time now on this deployment's clock, to the microsecond, which is
/// what the databases keep.
pub fn now() -> DateTime<Utc> {
    Utc::now().trunc_subsecs(6)
}
