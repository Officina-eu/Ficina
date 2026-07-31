//! Large-file share links (Ficina Transfer): store a file as a blob and mint a
//! private, expiring download link for it, so a message can carry a link instead
//! of an oversized inline attachment. The link token is stored **hashed** (a DB
//! read never yields a live link); the public download path hashes the incoming
//! token to look the row up. Creation is account-scoped; resolution + reclaim
//! are cross-tenant maintenance on [`Store`] (the public route has no account).

use bytes::Bytes;

use crate::account::AccountStore;
use crate::blob::hash_hex;
use crate::error::{Result, StoreError};
use crate::id::{TenantId, generate_token};
use crate::store::Store;

/// A freshly created share: the raw token (goes into the link URL, shown once)
/// and when it expires.
#[derive(Debug, Clone)]
pub struct ShareCreated {
    pub token: String,
    pub size: i64,
    pub expires_at_epoch: i64,
}

/// A resolved (live, unexpired) share, enough to serve its bytes.
#[derive(Debug, Clone)]
pub struct ShareTarget {
    pub tenant: TenantId,
    pub blob_id: String,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
}

impl AccountStore {
    /// Store `bytes` as a blob and create an expiring share link for it. Returns
    /// the raw token (store it nowhere else — it lives only in the link) and the
    /// expiry. Enforces the blob-store size ceiling + the tenant storage quota
    /// (via `put_blob`). `expires_at_epoch` is Unix seconds.
    ///
    /// # Errors
    /// [`StoreError::TooLarge`] over the size ceiling; [`StoreError::OverQuota`]
    /// over quota; [`StoreError::Db`] on failure.
    pub async fn create_share(
        &self,
        bytes: Bytes,
        filename: &str,
        content_type: &str,
        expires_at_epoch: i64,
    ) -> Result<ShareCreated> {
        let size = bytes.len() as i64;
        // put_blob dedups per (tenant, hash), enforces the size ceiling + quota,
        // and records the blobs row the reclaim sweep later inspects.
        let blob_id = self.put_blob(bytes, Some(content_type)).await?;
        // 256-bit unguessable token (two 128-bit draws), stored hashed at rest.
        let token = format!("{}{}", generate_token(), generate_token());
        let token_hash = hash_hex(token.as_bytes());
        sqlx::query(
            "INSERT INTO file_shares \
                 (token_hash, tenant_id, user_id, blob_id, filename, content_type, size, expires_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, to_timestamp($8))",
        )
        .bind(&token_hash)
        .bind(self.tenant.as_str())
        .bind(self.user.as_str())
        .bind(blob_id.as_str())
        .bind(filename)
        .bind(content_type)
        .bind(size)
        .bind(expires_at_epoch)
        .execute(&self.pool)
        .await?;
        Ok(ShareCreated { token, size, expires_at_epoch })
    }
}

impl Store {
    /// Resolve a share token to its target if it exists and has not expired.
    /// Cross-tenant: the token itself identifies the owning tenant. Returns
    /// `None` for an unknown or expired token (indistinguishable to a caller —
    /// no oracle for "existed but expired").
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn resolve_share(&self, token: &str) -> Result<Option<ShareTarget>> {
        let token_hash = hash_hex(token.as_bytes());
        let row: Option<(String, String, String, String, i64)> = sqlx::query_as(
            "SELECT tenant_id, blob_id, filename, content_type, size FROM file_shares \
             WHERE token_hash = $1 AND expires_at > now()",
        )
        .bind(&token_hash)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(|(tenant, blob_id, filename, content_type, size)| ShareTarget {
            tenant: TenantId::new(tenant),
            blob_id,
            filename,
            content_type,
            size,
        }))
    }

    /// The bytes behind a resolved share. Looks up the blob's content hash within
    /// the share's tenant (never crosses tenants) and reads it from blob storage.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if the blob is gone; [`StoreError::Db`]/`Blob` on
    /// failure.
    pub async fn share_bytes(&self, target: &ShareTarget) -> Result<Bytes> {
        let hash: Option<String> =
            sqlx::query_scalar("SELECT hash FROM blobs WHERE tenant_id = $1 AND id = $2")
                .bind(target.tenant.as_str())
                .bind(&target.blob_id)
                .fetch_optional(self.pool())
                .await?;
        let hash = hash.ok_or(StoreError::NotFound)?;
        self.blobs().get(target.tenant.as_str(), &hash).await
    }

    /// Delete every expired share, which immediately disables its link. Returns
    /// how many shares were expired. Cross-tenant maintenance, safe on an
    /// interval.
    ///
    /// This intentionally does **not** delete the underlying blob bytes.
    /// Blobs are content-addressed and deduplicated per `(tenant, hash)`, so a
    /// share's blob may be the very same row a message attachment (or another
    /// share) relies on, and the current `refcount` does not reliably
    /// distinguish "unreferenced" from "uploaded but not yet embedded". Deleting
    /// on that basis would risk silent data loss, so reclamation is deferred to a
    /// dedicated blob GC with a proper reference model (tracked follow-up). Until
    /// then an expired share leaks its bytes exactly like any other
    /// currently-unreferenced blob — no worse, and never destructive.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn sweep_expired_shares(&self) -> Result<usize> {
        let deleted = sqlx::query("DELETE FROM file_shares WHERE expires_at <= now()")
            .execute(self.pool())
            .await?
            .rows_affected();
        Ok(deleted as usize)
    }
}
