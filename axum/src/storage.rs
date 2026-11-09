//! The bucket uploads go to, chosen at start from the environment:
//! `STORAGE_BUCKET` unset, there is none, and the upload routes answer
//! 503; set, the S3 API at `STORAGE_ENDPOINT` (AWS when unset) in
//! `STORAGE_REGION`, with `STORAGE_ACCESS_KEY_ID` and
//! `STORAGE_SECRET_ACCESS_KEY`, and the files read by anyone at
//! `STORAGE_PUBLIC_URL`. `bin/storage` runs Garage for dev and sets them.

use std::env;

use app_core::storage::{S3Settings, Storage};
use secrecy::SecretString;

pub fn open() -> Result<Option<Storage>, String> {
    let Some(bucket) = env::var("STORAGE_BUCKET")
        .ok()
        .filter(|bucket| !bucket.is_empty())
    else {
        tracing::info!("storage: none, STORAGE_BUCKET is unset, so uploads are off");
        return Ok(None);
    };
    let required = |name: &str| {
        env::var(name)
            .ok()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("STORAGE_BUCKET is set, so {name} is needed too"))
    };
    let settings = S3Settings {
        endpoint: env::var("STORAGE_ENDPOINT")
            .ok()
            .filter(|value| !value.is_empty()),
        region: required("STORAGE_REGION")?,
        access_key_id: required("STORAGE_ACCESS_KEY_ID")?,
        secret_access_key: SecretString::from(required("STORAGE_SECRET_ACCESS_KEY")?),
        public_url: required("STORAGE_PUBLIC_URL")?,
        bucket,
    };
    tracing::info!(
        "storage: bucket {} at {}",
        settings.bucket,
        settings.endpoint.as_deref().unwrap_or("AWS")
    );
    Storage::s3(settings)
        .map(Some)
        .map_err(|error| error.to_string())
}
