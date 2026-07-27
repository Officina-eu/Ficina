//! Content-addressed blob storage over Garage (S3), abstracted through
//! `object_store` so tests run against an in-memory backend and
//! production against Garage without a code change.
//!
//! The SHA-256 of the bytes is the content id; the object key is
//! `<tenant_id>/<hash>`, so a tenant's blobs are physically prefixed by
//! its id and a `TenantStore` only ever reads under its own prefix. A
//! byte ceiling is enforced on write *and* on read.

use std::sync::Arc;

use bytes::Bytes;
use object_store::memory::InMemory;
use object_store::path::Path as ObjectPath;
use object_store::{ObjectStore, PutPayload};
use sha2::{Digest, Sha256};

use crate::error::{Result, StoreError};

/// Connection settings for a Garage (S3) backend.
#[cfg(feature = "garage")]
#[derive(Debug, Clone)]
pub struct GarageConfig {
    /// S3 endpoint URL (e.g. `http://garage:3900`).
    pub endpoint: String,
    /// Access key id.
    pub access_key: String,
    /// Secret access key.
    pub secret_key: String,
    /// Bucket holding all tenants' blobs (isolation is by key prefix).
    pub bucket: String,
    /// S3 region label (Garage accepts any; `garage` is conventional).
    pub region: String,
    /// Allow plaintext HTTP (Garage behind our network boundary).
    pub allow_http: bool,
}

/// A content-addressed blob store with a per-object byte ceiling.
#[derive(Clone)]
pub struct BlobStore {
    inner: Arc<dyn ObjectStore>,
    max_size: usize,
}

impl std::fmt::Debug for BlobStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlobStore")
            .field("max_size", &self.max_size)
            .finish_non_exhaustive()
    }
}

impl BlobStore {
    /// An in-memory backend for tests (still exercises the tenant-prefix
    /// and content-addressing logic).
    pub fn in_memory(max_size: usize) -> Self {
        Self {
            inner: Arc::new(InMemory::new()),
            max_size,
        }
    }

    /// A Garage (S3) backend (requires the `garage` feature).
    ///
    /// # Errors
    /// [`StoreError::Blob`] when the S3 client cannot be built.
    #[cfg(feature = "garage")]
    pub fn garage(config: &GarageConfig, max_size: usize) -> Result<Self> {
        use object_store::aws::AmazonS3Builder;
        let s3 = AmazonS3Builder::new()
            .with_endpoint(&config.endpoint)
            .with_access_key_id(&config.access_key)
            .with_secret_access_key(&config.secret_key)
            .with_bucket_name(&config.bucket)
            .with_region(&config.region)
            .with_allow_http(config.allow_http)
            .with_virtual_hosted_style_request(false) // Garage uses path style
            .build()?;
        Ok(Self {
            inner: Arc::new(s3),
            max_size,
        })
    }

    /// The configured byte ceiling.
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// Stores `bytes` under `<tenant>/<hash>`. Idempotent for identical
    /// content (content-addressed). Enforces the size ceiling on write.
    ///
    /// # Errors
    /// [`StoreError::TooLarge`] over the ceiling; [`StoreError::Blob`] on
    /// an S3 failure.
    pub async fn put(&self, tenant: &str, hash: &str, bytes: Bytes) -> Result<()> {
        if bytes.len() > self.max_size {
            return Err(StoreError::TooLarge {
                size: bytes.len(),
                limit: self.max_size,
            });
        }
        self.inner
            .put(&key(tenant, hash), PutPayload::from_bytes(bytes))
            .await?;
        Ok(())
    }

    /// Fetches the bytes at `<tenant>/<hash>`, enforcing the ceiling on
    /// read as defence against a tampered/oversized object.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] when absent; [`StoreError::TooLarge`] if
    /// the stored object exceeds the ceiling; [`StoreError::Blob`]
    /// otherwise.
    pub async fn get(&self, tenant: &str, hash: &str) -> Result<Bytes> {
        let result = self.inner.get(&key(tenant, hash)).await?;
        // Reject on the object's declared size BEFORE buffering it, so an
        // oversized/tampered object is never fully read into memory.
        let declared = result.meta.size as usize;
        if declared > self.max_size {
            return Err(StoreError::TooLarge {
                size: declared,
                limit: self.max_size,
            });
        }
        let bytes = result.bytes().await?;
        // Belt-and-braces: the materialized length must also be within the
        // ceiling (guards a lying `meta.size`).
        if bytes.len() > self.max_size {
            return Err(StoreError::TooLarge {
                size: bytes.len(),
                limit: self.max_size,
            });
        }
        Ok(bytes)
    }

    /// Whether an object exists at `<tenant>/<hash>`.
    ///
    /// # Errors
    /// [`StoreError::Blob`] on an S3 failure other than not-found.
    pub async fn exists(&self, tenant: &str, hash: &str) -> Result<bool> {
        match self.inner.head(&key(tenant, hash)).await {
            Ok(_) => Ok(true),
            Err(object_store::Error::NotFound { .. }) => Ok(false),
            Err(other) => Err(other.into()),
        }
    }

    /// Deletes an object (for the GC sweep — never on the delivery path).
    ///
    /// # Errors
    /// [`StoreError::Blob`] on an S3 failure.
    pub async fn delete(&self, tenant: &str, hash: &str) -> Result<()> {
        self.inner.delete(&key(tenant, hash)).await?;
        Ok(())
    }
}

/// The object key for a tenant's content-addressed blob. The tenant id
/// leads, so isolation is a physical key-prefix boundary.
fn key(tenant: &str, hash: &str) -> ObjectPath {
    ObjectPath::from(format!("{tenant}/{hash}"))
}

/// The SHA-256 of `data` as lowercase hex — the content id.
pub fn hash_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push(nibble(byte >> 4));
        out.push(nibble(byte & 0x0f));
    }
    out
}

fn nibble(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        _ => (b'a' + (n - 10)) as char,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn hash_is_stable_sha256_hex() {
        // Known SHA-256 of "abc".
        assert_eq!(
            hash_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[tokio::test]
    async fn put_get_roundtrip_and_tenant_prefix_isolation() {
        let store = BlobStore::in_memory(1024);
        let data = Bytes::from_static(b"hello world");
        let hash = hash_hex(&data);
        store.put("tenant-a", &hash, data.clone()).await.unwrap();

        assert_eq!(store.get("tenant-a", &hash).await.unwrap(), data);
        assert!(store.exists("tenant-a", &hash).await.unwrap());
        // Same content hash under a *different* tenant prefix is absent —
        // blobs do not leak across tenants even at identical hashes.
        assert!(!store.exists("tenant-b", &hash).await.unwrap());
        assert!(matches!(
            store.get("tenant-b", &hash).await,
            Err(StoreError::NotFound)
        ));
    }

    #[tokio::test]
    async fn oversize_write_is_rejected() {
        let store = BlobStore::in_memory(4);
        let err = store
            .put("t", "h", Bytes::from_static(b"12345"))
            .await
            .unwrap_err();
        assert!(matches!(err, StoreError::TooLarge { size: 5, limit: 4 }));
    }
}
