//! A deployment: the database this web server or app opened, and its
//! role. The web server is the system of record; a desktop or mobile app
//! is a device with a copy that syncs with it, see
//! `docs/design/offline-sync.md`. Reads are the same in both roles. A
//! write differs in two places: the server increments the row's version
//! and records the event in its log, a device leaves the version as the
//! server's and queues the event for the next push.

use chrono::{DateTime, SubsecRound, Utc};

use crate::database::{Database, Transaction};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// The web server, the system of record.
    Server,
    /// A desktop or mobile app, syncing its copy with the server.
    Device,
}

#[derive(Clone, Debug)]
pub struct Deployment {
    pub database: Database,
    pub role: Role,
}

impl Deployment {
    pub fn new(database: Database, role: Role) -> Deployment {
        Deployment { database, role }
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
