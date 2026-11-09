//! Object storage: an S3-style bucket the server hands out signed upload
//! URLs for, so a file goes from the person's device straight to the
//! bucket and never through the server. Server only: a device holds no
//! keys and asks the server, see `avatars`. In dev the bucket is Garage's
//! (`bin/storage`); in tests it is in memory.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use futures_util::TryStreamExt;

use object_store::aws::{AmazonS3, AmazonS3Builder};
use object_store::memory::InMemory;
use object_store::path::Path;
use object_store::signer::{HeaderValue, Method, SignedUrlOptions, Signer};
use object_store::{ObjectStore, ObjectStoreExt, PutPayload};
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

/// A file in the bucket: its key, and when it was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredFile {
    pub key: String,
    pub last_modified: DateTime<Utc>,
}

#[derive(Debug, Error)]
#[error("object storage: {0}")]
pub struct StorageError(String);

impl From<object_store::Error> for StorageError {
    fn from(error: object_store::Error) -> StorageError {
        StorageError(error.to_string())
    }
}

/// Where the server's bucket is and the keys it signs with.
pub struct S3Settings {
    /// The S3 API's URL for a store other than AWS, such as Garage or R2.
    pub endpoint: Option<String>,
    pub region: String,
    pub bucket: String,
    pub access_key_id: String,
    pub secret_access_key: SecretString,
    /// Where the bucket's objects are read from by anyone, such as a CDN,
    /// without a trailing slash.
    pub public_url: String,
}

#[derive(Clone, Debug)]
pub enum Storage {
    S3 {
        store: Arc<AmazonS3>,
        public_url: String,
    },
    /// For tests: the server's half works, and a test puts the files
    /// where a device's upload would have.
    Memory { store: Arc<InMemory> },
}

impl Storage {
    pub fn s3(settings: S3Settings) -> Result<Storage, StorageError> {
        let mut builder = AmazonS3Builder::new()
            .with_region(settings.region)
            .with_bucket_name(settings.bucket)
            .with_access_key_id(settings.access_key_id)
            .with_secret_access_key(settings.secret_access_key.expose_secret());
        if let Some(endpoint) = settings.endpoint {
            builder = builder
                .with_allow_http(endpoint.starts_with("http://"))
                .with_endpoint(endpoint);
        }
        Ok(Storage::S3 {
            store: Arc::new(builder.build()?),
            public_url: settings.public_url.trim_end_matches('/').to_string(),
        })
    }

    pub fn memory() -> Storage {
        Storage::Memory {
            store: Arc::new(InMemory::new()),
        }
    }

    /// A URL that lets whoever holds it PUT a file at the key until it
    /// expires, with this content type, which is signed, so the bucket
    /// refuses another. The size cannot be signed into a PUT URL here,
    /// so it is checked once the file is there.
    pub async fn upload_url(
        &self,
        key: &str,
        content_type: &str,
        lifetime: Duration,
    ) -> Result<String, StorageError> {
        match self {
            Storage::S3 { store, .. } => {
                let content_type = HeaderValue::from_str(content_type)
                    .map_err(|error| StorageError(error.to_string()))?;
                let options = SignedUrlOptions::new().with_signed_header(
                    object_store::signer::HeaderName::from_static("content-type"),
                    content_type,
                );
                let url = store
                    .signed_url_opts(Method::PUT, &Path::from(key), lifetime, &options)
                    .await?;
                Ok(url.to_string())
            }
            Storage::Memory { .. } => Ok(format!("memory:///{key}")),
        }
    }

    /// The size of the file at the key, or none when there is no file.
    pub async fn size(&self, key: &str) -> Result<Option<u64>, StorageError> {
        match self.store().head(&Path::from(key)).await {
            Ok(meta) => Ok(Some(meta.size)),
            Err(object_store::Error::NotFound { .. }) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// The file's first bytes, up to the count, to tell what it is.
    pub async fn first_bytes(
        &self,
        key: &str,
        count: u64,
        size: u64,
    ) -> Result<Vec<u8>, StorageError> {
        let bytes = self
            .store()
            .get_range(&Path::from(key), 0..count.min(size))
            .await?;
        Ok(bytes.to_vec())
    }

    pub async fn put(&self, key: &str, bytes: Vec<u8>) -> Result<(), StorageError> {
        self.store()
            .put(&Path::from(key), PutPayload::from(bytes))
            .await?;
        Ok(())
    }

    /// Every file whose key starts with the folder, `avatars` say.
    pub async fn list(&self, folder: &str) -> Result<Vec<StoredFile>, StorageError> {
        let files = self
            .store()
            .list(Some(&Path::from(folder)))
            .map_ok(|meta| StoredFile {
                key: meta.location.to_string(),
                last_modified: meta.last_modified,
            })
            .try_collect()
            .await?;
        Ok(files)
    }

    pub async fn delete(&self, key: &str) -> Result<(), StorageError> {
        self.store().delete(&Path::from(key)).await?;
        Ok(())
    }

    /// Where anyone reads the file at the key.
    pub fn public_url(&self, key: &str) -> String {
        match self {
            Storage::S3 { public_url, .. } => format!("{public_url}/{key}"),
            Storage::Memory { .. } => format!("memory:///{key}"),
        }
    }

    fn store(&self) -> &dyn ObjectStore {
        match self {
            Storage::S3 { store, .. } => store.as_ref(),
            Storage::Memory { store } => store.as_ref(),
        }
    }
}
