//! Recent correspondents, for compose recipient autocomplete. The address
//! fields on `messages` (`from_addr`, `to_addrs`, `cc_addrs`, `bcc_addrs`) hold
//! raw RFC 5322 header strings; this returns the most recent of them for the
//! account and leaves parsing + ranking to the caller (alo-jmap already owns
//! address-header parsing). Tenant/user-scoped like every other read.

use crate::account::AccountStore;
use crate::error::Result;

/// The raw address headers of one message, newest-first, for contact mining.
#[derive(Debug, Clone)]
pub struct AddressHeaders {
    pub from: String,
    pub to: String,
    pub cc: String,
    pub bcc: String,
}

impl AccountStore {
    /// The address headers of this account's most recent `limit` messages
    /// (newest first). The caller extracts individual addresses and ranks them;
    /// recency order is preserved so a caller can break ties by "seen most
    /// recently". Scoped to this (tenant, user).
    ///
    /// # Errors
    /// [`StoreError::Db`](crate::error::StoreError::Db) on failure.
    pub async fn recent_address_headers(&self, limit: i64) -> Result<Vec<AddressHeaders>> {
        let rows: Vec<(String, String, String, String)> = sqlx::query_as(
            "SELECT from_addr, to_addrs, cc_addrs, bcc_addrs FROM messages \
             WHERE tenant_id = $1 AND user_id = $2 \
             ORDER BY received_at DESC LIMIT $3",
        )
        .bind(self.tenant.as_str())
        .bind(self.user.as_str())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(from, to, cc, bcc)| AddressHeaders { from, to, cc, bcc })
            .collect())
    }
}
