//! Mail settings persistence (Law 3: kept out of `account.rs`/`store.rs`): a
//! per-user signature and the tenant-wide organization footer. Both are HTML
//! fragments the compose surface inserts; an unset value is the empty string.
//!
//! New table/column land in migration 0017 and are not in the offline query
//! cache, so these use the runtime `sqlx::query*` path.

use crate::account::AccountStore;
use crate::error::{Result, StoreError};
use crate::id::TenantId;
use crate::store::Store;

impl AccountStore {
    /// This user's mail signature (HTML), or empty if unset.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn signature(&self) -> Result<String> {
        let sig: Option<String> = sqlx::query_scalar(
            "SELECT signature FROM user_settings WHERE tenant_id = $1 AND user_id = $2",
        )
        .bind(self.tenant.as_str())
        .bind(self.user.as_str())
        .fetch_optional(&self.pool)
        .await?;
        Ok(sig.unwrap_or_default())
    }

    /// Sets this user's mail signature (HTML). Upsert; `updated_at` is bumped.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn set_signature(&self, signature: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO user_settings (tenant_id, user_id, signature) VALUES ($1, $2, $3) \
             ON CONFLICT (tenant_id, user_id) DO UPDATE SET signature = $3, updated_at = now()",
        )
        .bind(self.tenant.as_str())
        .bind(self.user.as_str())
        .bind(signature)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

impl Store {
    /// The tenant's organization footer (HTML), appended to outgoing mail, or
    /// empty if unset.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn org_footer(&self, tenant: &TenantId) -> Result<String> {
        let footer: Option<String> =
            sqlx::query_scalar("SELECT org_footer FROM tenants WHERE id = $1")
                .bind(tenant.as_str())
                .fetch_optional(self.pool())
                .await?;
        Ok(footer.unwrap_or_default())
    }

    /// Sets the tenant's organization footer (HTML). Admin-set (ADR 0012 gate
    /// enforced by the caller).
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if the tenant does not exist;
    /// [`StoreError::Db`] on failure.
    pub async fn set_org_footer(&self, tenant: &TenantId, footer: &str) -> Result<()> {
        let done = sqlx::query("UPDATE tenants SET org_footer = $2 WHERE id = $1")
            .bind(tenant.as_str())
            .bind(footer)
            .execute(self.pool())
            .await?;
        if done.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }
}
