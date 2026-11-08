//! Device settings: what a person chooses on a device, such as working
//! offline, kept in the device's database as a JSON value per key so a
//! new setting needs no migration. Every setting is declared here, with
//! the value a device starts with, and `upgrade` brings a device up to
//! date with that list when the app starts, so a setting is always there
//! to read. Never synced.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;

use crate::database::{Transaction, sql};
use crate::deployment::Deployment;

/// Whether the device works offline, making no request, see `sync`.
pub const WORK_OFFLINE: &str = "work_offline";

/// Every setting, with the value a device starts with. Add a setting
/// here, with its key as a constant above, and read it from the module
/// that owns it.
pub fn defaults() -> Vec<(&'static str, Value)> {
    vec![(WORK_OFFLINE, Value::Bool(false))]
}

#[derive(Debug, Error)]
pub enum DeviceSettingsError {
    #[error("device setting '{0}' was never set up")]
    Missing(String),
    #[error("device setting '{key}' holds something unexpected: {source}")]
    Invalid {
        key: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Brings the device's settings up to date with `defaults`: adds each
/// one that is missing with its first value, and leaves the rest as the
/// person set them. The place for a release that renames or reshapes a
/// setting to rewrite what an earlier release stored. Returns how many
/// were added.
pub async fn upgrade(deployment: &Deployment) -> Result<usize, DeviceSettingsError> {
    let mut tx = deployment.begin().await?;
    let mut added = 0;
    for (key, value) in defaults() {
        if sql!(&mut tx, device_settings::get(key))?.is_none() {
            sql!(&mut tx, device_settings::set(key, &value))?;
            added += 1;
        }
    }
    tx.commit().await?;
    Ok(added)
}

/// The setting's value, which `upgrade` made sure is there.
pub async fn get<T: DeserializeOwned>(
    tx: &mut Transaction,
    key: &str,
) -> Result<T, DeviceSettingsError> {
    let value = sql!(&mut *tx, device_settings::get(key))?
        .ok_or_else(|| DeviceSettingsError::Missing(key.to_string()))?;
    serde_json::from_value(value).map_err(|source| DeviceSettingsError::Invalid {
        key: key.to_string(),
        source,
    })
}

pub async fn set<T: Serialize>(
    tx: &mut Transaction,
    key: &str,
    value: &T,
) -> Result<(), DeviceSettingsError> {
    let value = serde_json::to_value(value).map_err(|source| DeviceSettingsError::Invalid {
        key: key.to_string(),
        source,
    })?;
    Ok(sql!(&mut *tx, device_settings::set(key, &value))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::with_each_database;
    use crate::deployment::Role;

    #[tokio::test]
    async fn device_settings_upgrade_adds_what_is_missing_and_keeps_what_is_set() {
        with_each_database(|database| async move {
            let deployment = Deployment::new(database, Role::Device);
            let mut tx = deployment.begin().await.expect("a transaction");
            let error = get::<bool>(&mut tx, WORK_OFFLINE)
                .await
                .expect_err("not set up");
            assert!(matches!(error, DeviceSettingsError::Missing(key) if key == WORK_OFFLINE));
            tx.commit().await.expect("committed");

            assert_eq!(
                upgrade(&deployment).await.expect("upgraded"),
                defaults().len()
            );
            let mut tx = deployment.begin().await.expect("a transaction");
            assert!(!get::<bool>(&mut tx, WORK_OFFLINE).await.expect("read"));
            set(&mut tx, WORK_OFFLINE, &true).await.expect("set");
            tx.commit().await.expect("committed");

            assert_eq!(upgrade(&deployment).await.expect("upgraded again"), 0);
            let mut tx = deployment.begin().await.expect("a transaction");
            assert!(get::<bool>(&mut tx, WORK_OFFLINE).await.expect("read"));
            let error = get::<String>(&mut tx, WORK_OFFLINE)
                .await
                .expect_err("not a string");
            assert!(matches!(error, DeviceSettingsError::Invalid { .. }));
            tx.commit().await.expect("committed");
        })
        .await;
    }
}
