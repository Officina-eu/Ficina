//! Inbound local delivery: carry a received message the last hop, SMTP →
//! `ficina-store`, with Sieve at the boundary. The one place `ficina-smtp`
//! meets the store and the outbound queue. Enabled only on the MX role when
//! a database URL is configured; the submission role and the outbound queue
//! are untouched. See `docs/design/local-delivery.md`.

use std::sync::Arc;

use ficina_store::{OutboundAction, Store};

use crate::envelope::Envelope;
use crate::error::SmtpError;
use crate::spool::Spool;

/// The result of delivering one message to its local recipients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryOutcome {
    /// Every recipient was filed (or intentionally discarded by Sieve).
    Delivered,
    /// A transient store/blob failure — the message is NOT accepted, so the
    /// sender must retry (no mail loss). Maps to a `4xx` at end of DATA.
    Transient,
}

/// Local delivery into the account store.
pub struct LocalDelivery {
    store: Arc<Store>,
    /// The inbound spool — reused to *enqueue* Sieve's outbound actions
    /// (redirect/vacation) for the existing outbound queue runner.
    spool: Arc<Spool>,
    hostname: String,
}

impl LocalDelivery {
    /// Connects the store for local delivery.
    ///
    /// # Errors
    /// [`SmtpError::Config`] if the blob directory or store cannot be opened.
    pub async fn connect(
        database_url: &str,
        blob_dir: &std::path::Path,
        spool: Arc<Spool>,
        hostname: String,
    ) -> Result<Self, SmtpError> {
        // A DURABLE on-disk blob backend so a delivered body survives a
        // restart (multi-node production swaps in Garage/S3 behind the
        // store's `garage` feature).
        let blobs = ficina_store::BlobStore::local(blob_dir, 50 * 1024 * 1024).map_err(|e| {
            SmtpError::Config {
                message: format!("local delivery: cannot open the blob store: {e}"),
            }
        })?;
        let store = Store::connect(database_url, blobs)
            .await
            .map_err(|e| SmtpError::Config {
                message: format!("local delivery: cannot connect to the store: {e}"),
            })?;
        store.migrate().await.map_err(|e| SmtpError::Config {
            message: format!("local delivery: store migration failed: {e}"),
        })?;
        Ok(Self::from_store(Arc::new(store), spool, hostname))
    }

    /// Builds local delivery over an existing store (used by embedders and
    /// tests that already hold a `Store`, so the same pool and blob backend
    /// are shared).
    pub fn from_store(store: Arc<Store>, spool: Arc<Spool>, hostname: String) -> Self {
        Self {
            store,
            spool,
            hostname,
        }
    }

    /// Whether `email` is a real local mailbox (for the RCPT-time check).
    /// Subaddress-aware: `user+tag@domain` resolves to `user@domain`.
    pub async fn recipient_exists(&self, email: &str) -> bool {
        matches!(self.resolve_account(email).await, Ok(Some(_)))
    }

    /// Resolves a recipient to its account, trying the address as-is then
    /// with any `+detail` stripped (subaddress, RFC 5233 — the mailbox is
    /// `user@domain`, the `+tag` is delivery detail the Sieve script tests).
    async fn resolve_account(
        &self,
        email: &str,
    ) -> Result<Option<(ficina_store::TenantId, ficina_store::UserId)>, ficina_store::StoreError>
    {
        if let Some(ids) = self.store.account_by_email(email).await? {
            return Ok(Some(ids));
        }
        match strip_subaddress(email) {
            Some(base) => self.store.account_by_email(&base).await,
            None => Ok(None),
        }
    }

    /// Delivers `message` to each local recipient through the account's
    /// Sieve script. Per-recipient and independent; a transient store fault
    /// for **any** recipient returns [`DeliveryOutcome::Transient`] (the
    /// conservative multi-recipient reply — RFC 5321 §6.1, duplicate over
    /// loss). Sieve's outbound actions are enqueued for the outbound queue.
    pub async fn deliver(
        &self,
        message: &[u8],
        mail_from: Option<&str>,
        rcpts: &[String],
    ) -> DeliveryOutcome {
        for rcpt in rcpts {
            let account = match self.resolve_account(rcpt).await {
                Ok(Some(ids)) => ids,
                // Accepted at RCPT but gone now (rare TOCTOU), or a DB error:
                // transient, so the sender retries rather than lose the mail.
                Ok(None) => {
                    tracing::warn!("recipient not found at DATA time; deferring");
                    return DeliveryOutcome::Transient;
                }
                Err(error) => {
                    tracing::error!(%error, "recipient lookup failed; deferring");
                    return DeliveryOutcome::Transient;
                }
            };
            let acc = self.store.for_account(account.0, account.1);
            match acc.deliver_sieve(message, mail_from, rcpt).await {
                Ok(delivery) => {
                    for warning in &delivery.warnings {
                        tracing::info!(warning = %warning, "sieve delivery warning");
                    }
                    // Enqueue redirect/vacation. An enqueue failure does NOT
                    // defer the whole message — the message IS filed; the
                    // outbound action is best-effort and logged.
                    for action in delivery.outbound {
                        if let Err(error) = self.enqueue(action, message, mail_from, rcpt).await {
                            tracing::error!(%error, "failed to enqueue sieve outbound action");
                        }
                    }
                }
                Err(error) => {
                    tracing::error!(%error, "store delivery failed; deferring");
                    return DeliveryOutcome::Transient;
                }
            }
        }
        DeliveryOutcome::Delivered
    }

    /// Enqueues a Sieve outbound action into the spool for the outbound
    /// queue runner. Attacker-influenced strings are CR/LF-stripped before
    /// any header is built (injection guard).
    async fn enqueue(
        &self,
        action: OutboundAction,
        message: &[u8],
        mail_from: Option<&str>,
        owner: &str,
    ) -> std::io::Result<()> {
        let (envelope, body) = match action {
            OutboundAction::Redirect { address } => {
                // Forward the original message; keep the original return-path
                // (avoids backscatter). The message already carries a
                // Received: stamp, so the store's loop ceiling bites on any
                // cycle back through us.
                let envelope = Envelope {
                    helo: self.hostname.clone(),
                    peer: "local-delivery".to_owned(),
                    mail_from: mail_from.map(str::to_owned),
                    rcpt_to: vec![strip_crlf(&address)],
                    received_at: jiff::Timestamp::now().to_string(),
                };
                (envelope, message.to_vec())
            }
            OutboundAction::Vacation {
                to,
                subject,
                from,
                reason,
            } => {
                let body =
                    build_vacation_reply(&to, subject.as_deref(), from.as_deref(), owner, &reason);
                // Null return-path (RFC 3834 §5) so the auto-reply can never
                // itself trigger a bounce loop.
                let envelope = Envelope {
                    helo: self.hostname.clone(),
                    peer: "local-delivery".to_owned(),
                    mail_from: None,
                    rcpt_to: vec![strip_crlf(&to)],
                    received_at: jiff::Timestamp::now().to_string(),
                };
                (envelope, body)
            }
        };
        let id = self.spool.next_id();
        let spool = Arc::clone(&self.spool);
        tokio::task::spawn_blocking(move || spool.store(&id, &envelope, &body))
            .await
            .map_err(std::io::Error::other)?
    }

    /// One-shot startup migration: any spooled message destined **entirely**
    /// for local recipients is delivered into the store and removed from the
    /// spool. Must run **before** the outbound queue runner starts (so there
    /// is no concurrent claim). Entries with a non-local recipient are left
    /// for the outbound queue. Returns the count migrated.
    ///
    /// # Errors
    /// [`std::io::Error`] if the spool cannot be listed.
    pub async fn migrate_spool(&self) -> std::io::Result<usize> {
        let ids = self.spool.list()?;
        let mut migrated = 0;
        for id in ids {
            let (envelope, message) = match self.spool.read(&id) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if envelope.rcpt_to.is_empty() {
                continue;
            }
            // All recipients must be local to migrate (else it is outbound).
            let mut all_local = true;
            for rcpt in &envelope.rcpt_to {
                if !self.recipient_exists(rcpt).await {
                    all_local = false;
                    break;
                }
            }
            if !all_local {
                continue;
            }
            if self
                .deliver(&message, envelope.mail_from.as_deref(), &envelope.rcpt_to)
                .await
                == DeliveryOutcome::Delivered
            {
                // Remove from the spool: claim (new→cur) then complete.
                if self.spool.claim(&id).is_ok() {
                    let _ = self.spool.complete(&id);
                    migrated += 1;
                }
            }
        }
        if migrated > 0 {
            tracing::info!(migrated, "migrated spooled local mail into the store");
        }
        Ok(migrated)
    }
}

/// Strips a `+detail` subaddress: `user+tag@domain` → `user@domain`.
/// `None` if there is no `+` in the local part.
fn strip_subaddress(email: &str) -> Option<String> {
    let (local, domain) = email.split_once('@')?;
    let (user, _tag) = local.split_once('+')?;
    Some(format!("{user}@{domain}"))
}

/// Strips CR and LF (and other controls) from a header/envelope value so an
/// attacker-influenced Sieve string cannot inject a header or SMTP command.
fn strip_crlf(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

/// Builds a vacation auto-reply message. All header values are CR/LF-safe.
fn build_vacation_reply(
    to: &str,
    subject: Option<&str>,
    from: Option<&str>,
    owner: &str,
    reason: &str,
) -> Vec<u8> {
    let from_hdr = strip_crlf(from.unwrap_or(owner));
    let to_hdr = strip_crlf(to);
    let subject_hdr = strip_crlf(subject.unwrap_or("Automatic reply"));
    let date = jiff::Zoned::now().strftime("%a, %d %b %Y %H:%M:%S %z");
    format!(
        "From: {from_hdr}\r\n\
         To: {to_hdr}\r\n\
         Subject: {subject_hdr}\r\n\
         Date: {date}\r\n\
         Auto-Submitted: auto-replied\r\n\
         MIME-Version: 1.0\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\
         \r\n\
         {reason}\r\n"
    )
    .into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_is_stripped_from_headers() {
        assert_eq!(strip_crlf("hi\r\nBcc: evil@x"), "hiBcc: evil@x");
        assert_eq!(strip_crlf("clean subject"), "clean subject");
    }

    #[test]
    fn vacation_reply_headers_are_injection_safe() {
        let body = build_vacation_reply(
            "victim@x.test\r\nBcc: leak@evil.test",
            Some("Re:\r\nX-Injected: yes"),
            None,
            "owner@x.test",
            "I am away",
        );
        let text = String::from_utf8_lossy(&body);
        // The injected content must not become a new header LINE (CR/LF
        // stripped, so it can only survive inline within a value).
        assert!(!text.contains("\r\nBcc:"), "{text}");
        assert!(!text.contains("\r\nX-Injected:"), "{text}");
        assert!(text.contains("Auto-Submitted: auto-replied"));
        assert!(text.contains("From: owner@x.test"));
    }
}
