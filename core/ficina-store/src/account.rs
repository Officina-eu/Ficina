//! `AccountStore` — account isolation by construction.
//!
//! Where [`TenantStore`](crate::TenantStore) narrows access to one
//! tenant, `AccountStore` narrows it further to one **user** within that
//! tenant. It is the **only** door to user-owned rows — messages,
//! mailboxes, threads, keywords, per-account change log, and the blobs a
//! user's mail references — obtained solely via
//! [`Store::for_account`](crate::Store::for_account). Every statement it
//! issues carries `tenant_id = $tenant AND user_id = $user` (or an
//! ownership join for the join tables that have no `user_id` of their
//! own), so a cross-account access is unrepresentable in the API and
//! returns `NotFound` in the data — the same promise `for_tenant` makes
//! for tenancy, now for accounts, enforced by the compiler rather than by
//! a caller remembering to call an `owns_*` guard first.

use bytes::Bytes;
use sqlx::PgPool;

use crate::blob::{BlobStore, hash_hex};
use crate::error::{Result, StoreError};
use crate::id::{BlobId, MailboxId, MessageId, TenantId, ThreadId, UserId};
use crate::message;
use crate::model::{Blob, EmailQuery, Mailbox, Message, MessageSummary, Page, SortDirection};
use crate::store::{MAX_KEYWORDS, MAX_KEYWORD_LEN, SEEN};
use crate::thread;

/// A handle scoped to one `(tenant, user)`. Holds both ids privately and
/// bakes them into every statement; no method accepts a tenant or user
/// argument. Cheap to clone. Construct only via
/// [`Store::for_account`](crate::Store::for_account).
#[derive(Clone)]
pub struct AccountStore {
    pub(crate) pool: PgPool,
    pub(crate) blobs: BlobStore,
    pub(crate) tenant: TenantId,
    pub(crate) user: UserId,
}

impl AccountStore {
    /// The tenant this handle is scoped to.
    pub fn tenant(&self) -> &TenantId {
        &self.tenant
    }

    /// The user (JMAP account) this handle is scoped to.
    pub fn user(&self) -> &UserId {
        &self.user
    }

    // ---- ownership guards ---------------------------------------------
    // Each confirms a row is *this account's* — the id belongs to this
    // tenant AND this user. A foreign or cross-user id is `NotFound`,
    // never an oracle. These replace the former free-standing `owns_*`
    // guards on `TenantStore`: the scoping is now inside every method, so
    // there is no guard for a caller to forget.

    /// Confirms this account's user exists (a stale token after a user is
    /// deleted must not create orphan rows). `NotFound` otherwise.
    async fn assert_user(&self) -> Result<()> {
        sqlx::query!(
            "SELECT 1 AS one FROM users WHERE tenant_id = $1 AND id = $2",
            self.tenant.as_str(),
            self.user.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)
        .map(|_| ())
    }

    /// Confirms a mailbox is this account's. `NotFound` otherwise.
    async fn assert_owned_mailbox(&self, mailbox: &MailboxId) -> Result<()> {
        sqlx::query!(
            "SELECT 1 AS one FROM mailboxes WHERE tenant_id = $1 AND user_id = $2 AND id = $3",
            self.tenant.as_str(),
            self.user.as_str(),
            mailbox.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)
        .map(|_| ())
    }

    /// Confirms a message is this account's. `NotFound` otherwise.
    async fn assert_owned_message(&self, message: &MessageId) -> Result<()> {
        sqlx::query!(
            "SELECT 1 AS one FROM messages WHERE tenant_id = $1 AND user_id = $2 AND id = $3",
            self.tenant.as_str(),
            self.user.as_str(),
            message.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)
        .map(|_| ())
    }

    /// Locks this account's message row `FOR UPDATE` inside `tx` (also an
    /// account-scoped existence check). All counter-affecting operations
    /// take this lock so `$seen`-state and membership changes serialize —
    /// preventing a stale-delta counter drift. `NotFound` if the message
    /// is absent, foreign, or another user's.
    async fn lock_owned_message(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        message: &MessageId,
    ) -> Result<()> {
        sqlx::query!(
            "SELECT 1 AS one FROM messages \
             WHERE tenant_id = $1 AND user_id = $2 AND id = $3 FOR UPDATE",
            self.tenant.as_str(),
            self.user.as_str(),
            message.as_str()
        )
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(StoreError::NotFound)
        .map(|_| ())
    }

    /// Records this account's object changes and bumps the tenant modseq
    /// within `tx`. Every change belongs to this user by construction.
    async fn record(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        changes: &[crate::changes::Change<'_>],
    ) -> Result<i64> {
        crate::changes::bump_and_record(tx, self.tenant.as_str(), self.user.as_str(), changes).await
    }

    /// Mailbox ids a message is a member of — used to cascade change
    /// records and counter adjustments. The message is already confirmed
    /// this account's by the caller (it holds the row lock), and a
    /// message's memberships are all its own account's mailboxes.
    async fn message_mailbox_ids(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        message: &MessageId,
    ) -> Result<Vec<String>> {
        let rows = sqlx::query!(
            "SELECT mailbox_id FROM mailbox_messages WHERE tenant_id = $1 AND message_id = $2",
            self.tenant.as_str(),
            message.as_str()
        )
        .fetch_all(&mut **tx)
        .await?;
        Ok(rows.into_iter().map(|r| r.mailbox_id).collect())
    }

    async fn message_is_seen(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        message: &MessageId,
    ) -> Result<bool> {
        let row = sqlx::query!(
            "SELECT 1 AS one FROM message_keywords \
             WHERE tenant_id = $1 AND message_id = $2 AND keyword = $3",
            self.tenant.as_str(),
            message.as_str(),
            SEEN
        )
        .fetch_optional(&mut **tx)
        .await?;
        Ok(row.is_some())
    }

    // ---- change tracking ----------------------------------------------

    /// The account's current JMAP state (the tenant modseq) as an opaque
    /// token.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn state(&self) -> Result<String> {
        Ok(
            crate::changes::current_state(&self.pool, self.tenant.as_str())
                .await?
                .to_string(),
        )
    }

    /// Computes `/changes` for an object type since `since` (a raw
    /// modseq), bounded by `max`, scoped to this account.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn changes(&self, obj_type: &str, since: i64, max: i64) -> Result<crate::Changes> {
        crate::changes::changes_since(
            &self.pool,
            self.tenant.as_str(),
            self.user.as_str(),
            obj_type,
            since,
            max,
        )
        .await
    }

    // ---- mailboxes -----------------------------------------------------

    /// Creates a mailbox for this account, optionally under `parent` and
    /// with a JMAP `role`.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if `parent` is not this account's;
    /// [`StoreError::Conflict`] on a duplicate sibling name or role.
    pub async fn create_mailbox(
        &self,
        parent: Option<&MailboxId>,
        name: &str,
        role: Option<&str>,
    ) -> Result<MailboxId> {
        self.assert_user().await?;
        if let Some(parent) = parent {
            self.assert_owned_mailbox(parent).await?;
        }
        let id = MailboxId::generate();
        let mut tx = self.pool.begin().await.map_err(StoreError::Db)?;
        sqlx::query!(
            "INSERT INTO mailboxes (id, tenant_id, user_id, parent_id, name, role) \
             VALUES ($1, $2, $3, $4, $5, $6)",
            id.as_str(),
            self.tenant.as_str(),
            self.user.as_str(),
            parent.map(MailboxId::as_str),
            name,
            role
        )
        .execute(&mut *tx)
        .await?;
        self.record(
            &mut tx,
            &[crate::changes::Change::created(
                crate::changes::TYPE_MAILBOX,
                id.as_str(),
            )],
        )
        .await?;
        tx.commit().await.map_err(StoreError::Db)?;
        Ok(id)
    }

    /// Gets-or-creates this account's `inbox` role mailbox.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn inbox(&self) -> Result<MailboxId> {
        self.assert_user().await?;
        if let Some(row) = sqlx::query!(
            "SELECT id FROM mailboxes WHERE tenant_id = $1 AND user_id = $2 AND role = 'inbox'",
            self.tenant.as_str(),
            self.user.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        {
            return Ok(MailboxId::new(row.id));
        }
        self.create_mailbox(None, "Inbox", Some("inbox")).await
    }

    /// Fetches one of this account's mailboxes. Foreign/cross-user →
    /// `NotFound`.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if absent or not this account's.
    pub async fn mailbox(&self, id: &MailboxId) -> Result<Mailbox> {
        let row = sqlx::query!(
            "SELECT id, parent_id, name, role, total_messages, unread_messages \
             FROM mailboxes WHERE tenant_id = $1 AND user_id = $2 AND id = $3",
            self.tenant.as_str(),
            self.user.as_str(),
            id.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;
        Ok(Mailbox {
            id: MailboxId::new(row.id),
            parent_id: row.parent_id.map(MailboxId::new),
            name: row.name,
            role: row.role,
            total_messages: row.total_messages,
            unread_messages: row.unread_messages,
        })
    }

    /// Lists this account's mailboxes (paginated).
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn mailboxes(&self, page: Page) -> Result<Vec<Mailbox>> {
        let rows = sqlx::query!(
            "SELECT id, parent_id, name, role, total_messages, unread_messages \
             FROM mailboxes WHERE tenant_id = $1 AND user_id = $2 \
             ORDER BY name LIMIT $3 OFFSET $4",
            self.tenant.as_str(),
            self.user.as_str(),
            page.limit(),
            page.offset()
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| Mailbox {
                id: MailboxId::new(row.id),
                parent_id: row.parent_id.map(MailboxId::new),
                name: row.name,
                role: row.role,
                total_messages: row.total_messages,
                unread_messages: row.unread_messages,
            })
            .collect())
    }

    // ---- ingestion -----------------------------------------------------

    /// Delivers a raw message into this account's inbox (the
    /// SMTP/migration path). Convenience over [`Self::ingest`].
    ///
    /// # Errors
    /// See [`Self::ingest`].
    pub async fn deliver(&self, raw: &[u8]) -> Result<MessageId> {
        let inbox = self.inbox().await?;
        self.ingest(&inbox, raw).await
    }

    /// Ingests a raw message into one of this account's mailboxes:
    /// content-address the bytes to the blob store (first — see
    /// crash-safety note), then in one transaction thread it, insert the
    /// row, add mailbox membership, bump counters, and build the search
    /// vector.
    ///
    /// # Errors
    /// [`StoreError::TooLarge`] over the blob ceiling;
    /// [`StoreError::NotFound`] if `mailbox` is not this account's;
    /// [`StoreError::Db`]/[`StoreError::Blob`] on failure.
    pub async fn ingest(&self, mailbox: &MailboxId, raw: &[u8]) -> Result<MessageId> {
        // Bound the size before any parse/copy/blob work.
        if raw.len() > self.blobs.max_size() {
            return Err(StoreError::TooLarge {
                size: raw.len(),
                limit: self.blobs.max_size(),
            });
        }
        // Reject a stale user, or a mailbox that is not this account's,
        // before writing any blob.
        self.assert_user().await?;
        self.assert_owned_mailbox(mailbox).await?;

        let parsed = message::parse(raw);
        let hash = hash_hex(raw);
        let size = raw.len() as i64;

        // Crash-safety: the blob is written BEFORE the DB commit. A crash
        // in between leaves an orphan blob no row references — invisible
        // to every tenant, reclaimed by GC — never a visible message with
        // a missing body.
        self.blobs
            .put(self.tenant.as_str(), &hash, Bytes::copy_from_slice(raw))
            .await?;

        let mut tx = self.pool.begin().await.map_err(StoreError::Db)?;

        // Upsert the blob row; refcount tracks referencing messages.
        let new_blob_id = BlobId::generate();
        let blob_row = sqlx::query!(
            "INSERT INTO blobs (id, tenant_id, hash, size, refcount, content_type) \
             VALUES ($1, $2, $3, $4, 1, 'message/rfc822') \
             ON CONFLICT (tenant_id, hash) DO UPDATE SET refcount = blobs.refcount + 1 \
             RETURNING id",
            new_blob_id.as_str(),
            self.tenant.as_str(),
            &hash,
            size
        )
        .fetch_one(&mut *tx)
        .await?;
        let blob_id = blob_row.id;

        // Thread: join the thread of any earlier message THIS account sent
        // that we reference (threads are per-account).
        let (thread_id, thread_created) = self
            .resolve_thread(&mut tx, &parsed.referenced_ids, &parsed.subject)
            .await?;

        let message_id = MessageId::generate();
        let search_text = format!(
            "{} {} {} {}",
            parsed.subject, parsed.from_addr, parsed.to_addrs, parsed.body_text
        );
        sqlx::query!(
            "INSERT INTO messages \
             (id, tenant_id, user_id, thread_id, blob_id, message_id_hdr, subject, from_addr, \
              to_addrs, sent_at, size, auth_spf, auth_dkim, auth_dmarc, auth_raw, search) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15, to_tsvector('simple',$16))",
            message_id.as_str(),
            self.tenant.as_str(),
            self.user.as_str(),
            thread_id.as_str(),
            blob_id,
            parsed.message_id.as_deref(),
            parsed.subject,
            parsed.from_addr,
            parsed.to_addrs,
            parsed.sent_at,
            size,
            parsed.auth_spf.as_deref(),
            parsed.auth_dkim.as_deref(),
            parsed.auth_dmarc.as_deref(),
            parsed.auth_raw.as_deref(),
            search_text
        )
        .execute(&mut *tx)
        .await?;

        // Membership + counters (a fresh message is unread).
        sqlx::query!(
            "INSERT INTO mailbox_messages (tenant_id, mailbox_id, message_id) VALUES ($1,$2,$3)",
            self.tenant.as_str(),
            mailbox.as_str(),
            message_id.as_str()
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "UPDATE mailboxes SET total_messages = total_messages + 1, \
             unread_messages = unread_messages + 1 WHERE tenant_id = $1 AND id = $2",
            self.tenant.as_str(),
            mailbox.as_str()
        )
        .execute(&mut *tx)
        .await?;

        // Record: Email created, its Thread created/updated, the target
        // Mailbox updated (its counters changed).
        use crate::changes::{Change, TYPE_EMAIL, TYPE_MAILBOX, TYPE_THREAD};
        let thread_change = if thread_created {
            Change::created(TYPE_THREAD, thread_id.as_str())
        } else {
            Change::updated(TYPE_THREAD, thread_id.as_str())
        };
        self.record(
            &mut tx,
            &[
                Change::created(TYPE_EMAIL, message_id.as_str()),
                thread_change,
                Change::updated(TYPE_MAILBOX, mailbox.as_str()),
            ],
        )
        .await?;

        tx.commit().await.map_err(StoreError::Db)?;
        // Boundary instrumentation — ids and size only, never body/PII.
        tracing::debug!(tenant = %self.tenant, message = %message_id, size, "ingested message");
        Ok(message_id)
    }

    /// Resolves the thread for a message: the thread of the earliest
    /// message it references, else a new thread keyed by base subject.
    /// Returns `(thread, created)` where `created` is true for a fresh
    /// thread (so ingestion can record the right change type). Referenced
    /// messages are matched within this account only (threads are
    /// per-account).
    async fn resolve_thread(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        referenced_ids: &[String],
        subject: &str,
    ) -> Result<(ThreadId, bool)> {
        if !referenced_ids.is_empty() {
            let existing = sqlx::query!(
                "SELECT thread_id FROM messages \
                 WHERE tenant_id = $1 AND user_id = $2 AND message_id_hdr = ANY($3::text[]) \
                 ORDER BY created_at LIMIT 1",
                self.tenant.as_str(),
                self.user.as_str(),
                referenced_ids
            )
            .fetch_optional(&mut **tx)
            .await?;
            if let Some(row) = existing {
                return Ok((ThreadId::new(row.thread_id), false));
            }
        }
        let thread_id = ThreadId::generate();
        sqlx::query!(
            "INSERT INTO threads (id, tenant_id, subject_base) VALUES ($1, $2, $3)",
            thread_id.as_str(),
            self.tenant.as_str(),
            thread::base_subject(subject)
        )
        .execute(&mut **tx)
        .await?;
        Ok((thread_id, true))
    }

    // ---- reading -------------------------------------------------------

    /// Lists a mailbox newest-first (paginated). The hot query — served
    /// by the `(tenant_id, mailbox_id, added_at DESC)` index. The join to
    /// `messages` on `user_id` confines results to this account, so a
    /// foreign mailbox id yields an empty list, never another user's mail.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn list_mailbox(&self, mailbox: &MailboxId, page: Page) -> Result<Vec<MessageSummary>> {
        let rows = sqlx::query!(
            "SELECT m.id, m.thread_id, m.subject, m.from_addr, m.sent_at, m.received_at, m.size \
             FROM mailbox_messages mm \
             JOIN messages m ON m.id = mm.message_id AND m.tenant_id = mm.tenant_id \
             WHERE mm.tenant_id = $1 AND mm.mailbox_id = $2 AND m.user_id = $3 \
             ORDER BY mm.added_at DESC LIMIT $4 OFFSET $5",
            self.tenant.as_str(),
            mailbox.as_str(),
            self.user.as_str(),
            page.limit(),
            page.offset()
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| MessageSummary {
                id: MessageId::new(row.id),
                thread_id: ThreadId::new(row.thread_id),
                subject: row.subject,
                from_addr: row.from_addr,
                sent_at: row.sent_at,
                received_at: row.received_at,
                size: row.size,
            })
            .collect())
    }

    /// Fetches one of this account's messages' metadata.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if absent or not this account's.
    pub async fn message(&self, id: &MessageId) -> Result<Message> {
        let row = sqlx::query!(
            "SELECT id, thread_id, blob_id, message_id_hdr, subject, from_addr, to_addrs, \
             sent_at, received_at, size, auth_spf, auth_dkim, auth_dmarc \
             FROM messages WHERE tenant_id = $1 AND user_id = $2 AND id = $3",
            self.tenant.as_str(),
            self.user.as_str(),
            id.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;
        Ok(Message {
            id: MessageId::new(row.id),
            thread_id: ThreadId::new(row.thread_id),
            blob_id: BlobId::new(row.blob_id),
            message_id_hdr: row.message_id_hdr,
            subject: row.subject,
            from_addr: row.from_addr,
            to_addrs: row.to_addrs,
            sent_at: row.sent_at,
            received_at: row.received_at,
            size: row.size,
            auth_spf: row.auth_spf,
            auth_dkim: row.auth_dkim,
            auth_dmarc: row.auth_dmarc,
        })
    }

    /// Fetches one of this account's messages' raw bytes from the blob
    /// store (the blob hash is resolved via this account's own message
    /// row).
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if the message is absent or not this
    /// account's; [`StoreError::Blob`] on a blob failure.
    pub async fn message_bytes(&self, id: &MessageId) -> Result<Bytes> {
        let row = sqlx::query!(
            "SELECT b.hash FROM messages m JOIN blobs b ON b.id = m.blob_id \
             WHERE m.tenant_id = $1 AND m.user_id = $2 AND m.id = $3",
            self.tenant.as_str(),
            self.user.as_str(),
            id.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;
        self.blobs.get(self.tenant.as_str(), &row.hash).await
    }

    /// The keywords set on one of this account's messages. The join to
    /// `messages` on `user_id` confines the read to this account.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn keywords(&self, message: &MessageId) -> Result<Vec<String>> {
        let rows = sqlx::query!(
            "SELECT mk.keyword FROM message_keywords mk \
             JOIN messages m ON m.id = mk.message_id AND m.tenant_id = mk.tenant_id \
             WHERE mk.tenant_id = $1 AND mk.message_id = $2 AND m.user_id = $3 \
             ORDER BY mk.keyword",
            self.tenant.as_str(),
            message.as_str(),
            self.user.as_str()
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.keyword).collect())
    }

    // ---- flags & state -------------------------------------------------

    /// Sets or clears a keyword on one of this account's messages,
    /// maintaining the unread counter of every mailbox the message is in
    /// transactionally.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if the message is absent or not this
    /// account's.
    pub async fn set_keyword(&self, message: &MessageId, keyword: &str, on: bool) -> Result<()> {
        if on && keyword.len() > MAX_KEYWORD_LEN {
            return Err(StoreError::Conflict("keyword too long".to_owned()));
        }
        let mut tx = self.pool.begin().await.map_err(StoreError::Db)?;

        // Lock the message row (account-scoped existence check + serializes
        // against add/remove_from_mailbox so the unread delta is never
        // stale).
        self.lock_owned_message(&mut tx, message).await?;

        let changed = if on {
            let affected = sqlx::query!(
                "INSERT INTO message_keywords (tenant_id, message_id, keyword) VALUES ($1,$2,$3) \
                 ON CONFLICT DO NOTHING",
                self.tenant.as_str(),
                message.as_str(),
                keyword
            )
            .execute(&mut *tx)
            .await?
            .rows_affected();
            // Enforce the per-message keyword cap only when a genuinely new
            // keyword was added; the rollback keeps it from persisting.
            if affected == 1 {
                let count = sqlx::query!(
                    "SELECT count(*) AS n FROM message_keywords \
                     WHERE tenant_id = $1 AND message_id = $2",
                    self.tenant.as_str(),
                    message.as_str()
                )
                .fetch_one(&mut *tx)
                .await?
                .n
                .unwrap_or(0);
                if count > MAX_KEYWORDS {
                    return Err(StoreError::Conflict("too many keywords".to_owned()));
                }
            }
            affected
        } else {
            sqlx::query!(
                "DELETE FROM message_keywords WHERE tenant_id = $1 AND message_id = $2 AND keyword = $3",
                self.tenant.as_str(),
                message.as_str(),
                keyword
            )
            .execute(&mut *tx)
            .await?
            .rows_affected()
        };

        // Only $seen moves the unread counter, and only when the keyword
        // actually changed (rows_affected == 1) — so concurrent identical
        // updates cannot double-count.
        if keyword == SEEN && changed == 1 {
            // Adding $seen makes a message read (unread -1); removing it
            // makes it unread again (unread +1).
            let delta: i64 = if on { -1 } else { 1 };
            sqlx::query!(
                "UPDATE mailboxes SET unread_messages = unread_messages + $1 \
                 WHERE tenant_id = $2 AND id IN \
                 (SELECT mailbox_id FROM mailbox_messages WHERE tenant_id = $2 AND message_id = $3)",
                delta,
                self.tenant.as_str(),
                message.as_str()
            )
            .execute(&mut *tx)
            .await?;
        }

        // Record the Email change and, when $seen moved counters, the
        // affected mailboxes (their unread changed).
        if changed == 1 {
            use crate::changes::{Change, TYPE_EMAIL, TYPE_MAILBOX};
            let mut records = vec![Change::updated(TYPE_EMAIL, message.as_str())];
            let mailbox_ids = if keyword == SEEN {
                self.message_mailbox_ids(&mut tx, message).await?
            } else {
                Vec::new()
            };
            for mb in &mailbox_ids {
                records.push(Change::updated(TYPE_MAILBOX, mb));
            }
            self.record(&mut tx, &records).await?;
        }

        tx.commit().await.map_err(StoreError::Db)?;
        Ok(())
    }

    /// Adds one of this account's messages to one of its mailboxes
    /// (idempotent), bumping counters when it was not already a member.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if the message or mailbox is absent or not
    /// this account's.
    pub async fn add_to_mailbox(&self, message: &MessageId, mailbox: &MailboxId) -> Result<()> {
        // Message and mailbox must both be this account's.
        self.assert_owned_message(message).await?;
        self.assert_owned_mailbox(mailbox).await?;
        let mut tx = self.pool.begin().await.map_err(StoreError::Db)?;
        // Lock serializes the seen-state read against set_keyword.
        self.lock_owned_message(&mut tx, message).await?;
        let seen = self.message_is_seen(&mut tx, message).await?;
        let added = sqlx::query!(
            "INSERT INTO mailbox_messages (tenant_id, mailbox_id, message_id) \
             SELECT $1, $2, id FROM messages WHERE tenant_id = $1 AND id = $3 \
             ON CONFLICT DO NOTHING",
            self.tenant.as_str(),
            mailbox.as_str(),
            message.as_str()
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if added == 1 {
            let unread_delta: i64 = if seen { 0 } else { 1 };
            sqlx::query!(
                "UPDATE mailboxes SET total_messages = total_messages + 1, \
                 unread_messages = unread_messages + $1 WHERE tenant_id = $2 AND id = $3",
                unread_delta,
                self.tenant.as_str(),
                mailbox.as_str()
            )
            .execute(&mut *tx)
            .await?;
            use crate::changes::{Change, TYPE_EMAIL, TYPE_MAILBOX};
            self.record(
                &mut tx,
                &[
                    Change::updated(TYPE_EMAIL, message.as_str()),
                    Change::updated(TYPE_MAILBOX, mailbox.as_str()),
                ],
            )
            .await?;
        }
        tx.commit().await.map_err(StoreError::Db)?;
        Ok(())
    }

    /// Removes one of this account's messages from one of its mailboxes,
    /// adjusting counters when it was a member.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if the message or mailbox is absent or not
    /// this account's.
    pub async fn remove_from_mailbox(&self, message: &MessageId, mailbox: &MailboxId) -> Result<()> {
        self.assert_owned_message(message).await?;
        self.assert_owned_mailbox(mailbox).await?;
        let mut tx = self.pool.begin().await.map_err(StoreError::Db)?;
        self.lock_owned_message(&mut tx, message).await?;
        let seen = self.message_is_seen(&mut tx, message).await?;
        let removed = sqlx::query!(
            "DELETE FROM mailbox_messages WHERE tenant_id = $1 AND mailbox_id = $2 AND message_id = $3",
            self.tenant.as_str(),
            mailbox.as_str(),
            message.as_str()
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if removed == 1 {
            let unread_delta: i64 = if seen { 0 } else { -1 };
            sqlx::query!(
                "UPDATE mailboxes SET total_messages = total_messages - 1, \
                 unread_messages = unread_messages + $1 WHERE tenant_id = $2 AND id = $3",
                unread_delta,
                self.tenant.as_str(),
                mailbox.as_str()
            )
            .execute(&mut *tx)
            .await?;
            use crate::changes::{Change, TYPE_EMAIL, TYPE_MAILBOX};
            self.record(
                &mut tx,
                &[
                    Change::updated(TYPE_EMAIL, message.as_str()),
                    Change::updated(TYPE_MAILBOX, mailbox.as_str()),
                ],
            )
            .await?;
        }
        tx.commit().await.map_err(StoreError::Db)?;
        Ok(())
    }

    // ---- search --------------------------------------------------------

    /// Full-text search over this account's messages
    /// (subject/addresses/body), paginated.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn search(&self, query: &str, page: Page) -> Result<Vec<MessageSummary>> {
        let rows = sqlx::query!(
            "SELECT id, thread_id, subject, from_addr, sent_at, received_at, size \
             FROM messages \
             WHERE tenant_id = $1 AND user_id = $2 AND search @@ plainto_tsquery('simple', $3) \
             ORDER BY received_at DESC LIMIT $4 OFFSET $5",
            self.tenant.as_str(),
            self.user.as_str(),
            query,
            page.limit(),
            page.offset()
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| MessageSummary {
                id: MessageId::new(row.id),
                thread_id: ThreadId::new(row.thread_id),
                subject: row.subject,
                from_addr: row.from_addr,
                sent_at: row.sent_at,
                received_at: row.received_at,
                size: row.size,
            })
            .collect())
    }

    /// The message ids in a thread (this account's messages only, since
    /// threads are per-account), oldest first.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn thread_messages(&self, thread: &ThreadId, page: Page) -> Result<Vec<MessageId>> {
        let rows = sqlx::query!(
            "SELECT id FROM messages WHERE tenant_id = $1 AND user_id = $2 AND thread_id = $3 \
             ORDER BY created_at LIMIT $4 OFFSET $5",
            self.tenant.as_str(),
            self.user.as_str(),
            thread.as_str(),
            page.limit(),
            page.offset()
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| MessageId::new(r.id)).collect())
    }

    // ---- mailbox mutations (Mailbox/set) -------------------------------

    /// Renames one of this account's mailboxes. Records a Mailbox change.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if absent or not this account's;
    /// [`StoreError::Conflict`] on a duplicate sibling name.
    pub async fn rename_mailbox(&self, id: &MailboxId, name: &str) -> Result<()> {
        self.assert_owned_mailbox(id).await?;
        let mut tx = self.pool.begin().await.map_err(StoreError::Db)?;
        sqlx::query!(
            "UPDATE mailboxes SET name = $4 WHERE tenant_id = $1 AND user_id = $2 AND id = $3",
            self.tenant.as_str(),
            self.user.as_str(),
            id.as_str(),
            name
        )
        .execute(&mut *tx)
        .await?;
        self.record(
            &mut tx,
            &[crate::changes::Change::updated(
                crate::changes::TYPE_MAILBOX,
                id.as_str(),
            )],
        )
        .await?;
        tx.commit().await.map_err(StoreError::Db)?;
        Ok(())
    }

    /// Moves one of this account's mailboxes under a new parent (`None` =
    /// root). Records a Mailbox change.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if the mailbox or parent is absent or not
    /// this account's; [`StoreError::Conflict`] if the move would create a
    /// cycle or clash.
    pub async fn move_mailbox(&self, id: &MailboxId, parent: Option<&MailboxId>) -> Result<()> {
        if let Some(parent) = parent {
            self.assert_owned_mailbox(parent).await?;
            if parent == id {
                return Err(StoreError::Conflict(
                    "mailbox cannot parent itself".to_owned(),
                ));
            }
        }
        self.assert_owned_mailbox(id).await?;
        let mut tx = self.pool.begin().await.map_err(StoreError::Db)?;
        sqlx::query!(
            "UPDATE mailboxes SET parent_id = $4 WHERE tenant_id = $1 AND user_id = $2 AND id = $3",
            self.tenant.as_str(),
            self.user.as_str(),
            id.as_str(),
            parent.map(MailboxId::as_str)
        )
        .execute(&mut *tx)
        .await?;
        self.record(
            &mut tx,
            &[crate::changes::Change::updated(
                crate::changes::TYPE_MAILBOX,
                id.as_str(),
            )],
        )
        .await?;
        tx.commit().await.map_err(StoreError::Db)?;
        Ok(())
    }

    /// Destroys one of this account's empty, childless mailboxes. Records
    /// a Mailbox tombstone.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if absent or not this account's;
    /// [`StoreError::Conflict`] (mapped to JMAP
    /// `mailboxHasEmail`/`mailboxHasChild`) if it still holds messages or
    /// sub-mailboxes.
    pub async fn destroy_mailbox(&self, id: &MailboxId) -> Result<()> {
        self.assert_owned_mailbox(id).await?;
        let mut tx = self.pool.begin().await.map_err(StoreError::Db)?;
        let has_child = sqlx::query!(
            "SELECT 1 AS one FROM mailboxes WHERE tenant_id = $1 AND parent_id = $2 LIMIT 1",
            self.tenant.as_str(),
            id.as_str()
        )
        .fetch_optional(&mut *tx)
        .await?
        .is_some();
        if has_child {
            return Err(StoreError::Conflict("mailbox has children".to_owned()));
        }
        let has_email = sqlx::query!(
            "SELECT 1 AS one FROM mailbox_messages WHERE tenant_id = $1 AND mailbox_id = $2 LIMIT 1",
            self.tenant.as_str(),
            id.as_str()
        )
        .fetch_optional(&mut *tx)
        .await?
        .is_some();
        if has_email {
            return Err(StoreError::Conflict("mailbox has emails".to_owned()));
        }
        sqlx::query!(
            "DELETE FROM mailboxes WHERE tenant_id = $1 AND user_id = $2 AND id = $3",
            self.tenant.as_str(),
            self.user.as_str(),
            id.as_str()
        )
        .execute(&mut *tx)
        .await?;
        self.record(
            &mut tx,
            &[crate::changes::Change::destroyed(
                crate::changes::TYPE_MAILBOX,
                id.as_str(),
            )],
        )
        .await?;
        tx.commit().await.map_err(StoreError::Db)?;
        Ok(())
    }

    // ---- email query / mutations --------------------------------------

    /// The mailbox ids one of this account's messages belongs to — for
    /// `Email/get` `mailboxIds`.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if the message is absent or not this
    /// account's.
    pub async fn mailboxes_of_message(&self, message: &MessageId) -> Result<Vec<MailboxId>> {
        self.assert_owned_message(message).await?;
        let rows = sqlx::query!(
            "SELECT mailbox_id FROM mailbox_messages WHERE tenant_id = $1 AND message_id = $2",
            self.tenant.as_str(),
            message.as_str()
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| MailboxId::new(r.mailbox_id))
            .collect())
    }

    /// `Email/query`: filters + `receivedAt` sort + bounded page, over
    /// this account's messages only.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn query_emails(&self, q: &EmailQuery) -> Result<Vec<MessageSummary>> {
        let f = &q.filter;
        let in_mailbox = f.in_mailbox.as_ref().map(MailboxId::as_str);
        // One statement with optional predicates; sort direction picks the
        // ORDER BY (it cannot be a bind parameter).
        let rows = match q.sort {
            SortDirection::Desc => sqlx::query!(
                r#"SELECT DISTINCT m.id, m.thread_id, m.subject, m.from_addr, m.sent_at,
                              m.received_at, m.size
                       FROM messages m
                       LEFT JOIN mailbox_messages mm
                         ON mm.message_id = m.id AND mm.tenant_id = m.tenant_id
                       WHERE m.tenant_id = $1 AND m.user_id = $2
                         AND ($3::text IS NULL OR mm.mailbox_id = $3)
                         AND ($4::text IS NULL OR m.from_addr ILIKE '%' || $4 || '%')
                         AND ($5::text IS NULL OR m.to_addrs ILIKE '%' || $5 || '%')
                         AND ($6::text IS NULL OR m.subject ILIKE '%' || $6 || '%')
                         AND ($7::text IS NULL OR m.search @@ plainto_tsquery('simple', $7))
                         AND ($8::timestamptz IS NULL OR m.received_at < $8)
                         AND ($9::timestamptz IS NULL OR m.received_at >= $9)
                         AND ($10::text IS NULL OR EXISTS
                              (SELECT 1 FROM message_keywords k
                               WHERE k.message_id = m.id AND k.keyword = $10))
                         AND ($11::text IS NULL OR NOT EXISTS
                              (SELECT 1 FROM message_keywords k
                               WHERE k.message_id = m.id AND k.keyword = $11))
                       ORDER BY m.received_at DESC, m.id DESC
                       LIMIT $12 OFFSET $13"#,
                self.tenant.as_str(),
                self.user.as_str(),
                in_mailbox,
                f.from,
                f.to,
                f.subject,
                f.text,
                f.before,
                f.after,
                f.has_keyword,
                f.not_keyword,
                q.page.limit(),
                q.page.offset()
            )
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|r| MessageSummary {
                id: MessageId::new(r.id),
                thread_id: ThreadId::new(r.thread_id),
                subject: r.subject,
                from_addr: r.from_addr,
                sent_at: r.sent_at,
                received_at: r.received_at,
                size: r.size,
            })
            .collect(),
            SortDirection::Asc => sqlx::query!(
                r#"SELECT DISTINCT m.id, m.thread_id, m.subject, m.from_addr, m.sent_at,
                              m.received_at, m.size
                       FROM messages m
                       LEFT JOIN mailbox_messages mm
                         ON mm.message_id = m.id AND mm.tenant_id = m.tenant_id
                       WHERE m.tenant_id = $1 AND m.user_id = $2
                         AND ($3::text IS NULL OR mm.mailbox_id = $3)
                         AND ($4::text IS NULL OR m.from_addr ILIKE '%' || $4 || '%')
                         AND ($5::text IS NULL OR m.to_addrs ILIKE '%' || $5 || '%')
                         AND ($6::text IS NULL OR m.subject ILIKE '%' || $6 || '%')
                         AND ($7::text IS NULL OR m.search @@ plainto_tsquery('simple', $7))
                         AND ($8::timestamptz IS NULL OR m.received_at < $8)
                         AND ($9::timestamptz IS NULL OR m.received_at >= $9)
                         AND ($10::text IS NULL OR EXISTS
                              (SELECT 1 FROM message_keywords k
                               WHERE k.message_id = m.id AND k.keyword = $10))
                         AND ($11::text IS NULL OR NOT EXISTS
                              (SELECT 1 FROM message_keywords k
                               WHERE k.message_id = m.id AND k.keyword = $11))
                       ORDER BY m.received_at ASC, m.id ASC
                       LIMIT $12 OFFSET $13"#,
                self.tenant.as_str(),
                self.user.as_str(),
                in_mailbox,
                f.from,
                f.to,
                f.subject,
                f.text,
                f.before,
                f.after,
                f.has_keyword,
                f.not_keyword,
                q.page.limit(),
                q.page.offset()
            )
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|r| MessageSummary {
                id: MessageId::new(r.id),
                thread_id: ThreadId::new(r.thread_id),
                subject: r.subject,
                from_addr: r.from_addr,
                sent_at: r.sent_at,
                received_at: r.received_at,
                size: r.size,
            })
            .collect(),
        };
        Ok(rows)
    }

    /// Destroys one of this account's messages everywhere: adjusts every
    /// containing mailbox's counters, deletes the row
    /// (membership/keywords cascade), and records the Email tombstone plus
    /// the affected Mailbox updates.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if absent or not this account's.
    pub async fn destroy_message(&self, message: &MessageId) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(StoreError::Db)?;
        self.lock_owned_message(&mut tx, message).await?;
        let seen = self.message_is_seen(&mut tx, message).await?;
        let mailbox_ids = self.message_mailbox_ids(&mut tx, message).await?;
        for mb in &mailbox_ids {
            let unread_delta: i64 = if seen { 0 } else { -1 };
            sqlx::query!(
                "UPDATE mailboxes SET total_messages = total_messages - 1, \
                 unread_messages = unread_messages + $1 WHERE tenant_id = $2 AND id = $3",
                unread_delta,
                self.tenant.as_str(),
                mb
            )
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query!(
            "DELETE FROM messages WHERE tenant_id = $1 AND user_id = $2 AND id = $3",
            self.tenant.as_str(),
            self.user.as_str(),
            message.as_str()
        )
        .execute(&mut *tx)
        .await?;
        use crate::changes::{Change, TYPE_EMAIL, TYPE_MAILBOX};
        let mut records = vec![Change::destroyed(TYPE_EMAIL, message.as_str())];
        for mb in &mailbox_ids {
            records.push(Change::updated(TYPE_MAILBOX, mb));
        }
        self.record(&mut tx, &records).await?;
        tx.commit().await.map_err(StoreError::Db)?;
        Ok(())
    }

    // ---- blobs (JMAP upload/download) ---------------------------------

    /// Stores an uploaded blob (content-addressed) and returns its id.
    /// Idempotent for identical content. Blobs are deduplicated per
    /// tenant; an uploaded blob becomes downloadable to this account only
    /// once one of its messages references it (see [`Self::blob_bytes`]).
    ///
    /// # Errors
    /// [`StoreError::TooLarge`] over the ceiling; [`StoreError::Db`]/
    /// [`StoreError::Blob`] on failure.
    pub async fn put_blob(&self, bytes: Bytes, content_type: Option<&str>) -> Result<BlobId> {
        if bytes.len() > self.blobs.max_size() {
            return Err(StoreError::TooLarge {
                size: bytes.len(),
                limit: self.blobs.max_size(),
            });
        }
        let hash = hash_hex(&bytes);
        let size = bytes.len() as i64;
        self.blobs.put(self.tenant.as_str(), &hash, bytes).await?;
        let new_id = BlobId::generate();
        let row = sqlx::query!(
            "INSERT INTO blobs (id, tenant_id, hash, size, refcount, content_type) \
             VALUES ($1, $2, $3, $4, 0, $5) \
             ON CONFLICT (tenant_id, hash) DO UPDATE SET content_type = COALESCE($5, blobs.content_type) \
             RETURNING id",
            new_id.as_str(),
            self.tenant.as_str(),
            &hash,
            size,
            content_type
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(BlobId::new(row.id))
    }

    /// A blob's metadata, scoped to this account: visible only when one of
    /// this account's messages references it. Blobs carry no `user_id` (they
    /// are content-addressed and tenant-deduplicated), so the ownership
    /// gate is an `EXISTS` over this account's messages — the same rule the
    /// former `owns_blob` guard enforced, now inseparable from the read.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if absent, foreign, or unreferenced by this
    /// account.
    pub async fn blob(&self, id: &BlobId) -> Result<Blob> {
        let row = sqlx::query!(
            "SELECT b.id, b.size, b.content_type FROM blobs b \
             WHERE b.tenant_id = $1 AND b.id = $2 AND EXISTS \
             (SELECT 1 FROM messages m \
              WHERE m.tenant_id = b.tenant_id AND m.blob_id = b.id AND m.user_id = $3)",
            self.tenant.as_str(),
            id.as_str(),
            self.user.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;
        Ok(Blob {
            id: BlobId::new(row.id),
            size: row.size,
            content_type: row.content_type,
        })
    }

    /// A blob's bytes, scoped to this account (see [`Self::blob`] for the
    /// ownership rule).
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if absent, foreign, or unreferenced by this
    /// account; [`StoreError::Blob`] on a blob-store failure.
    pub async fn blob_bytes(&self, id: &BlobId) -> Result<Bytes> {
        let row = sqlx::query!(
            "SELECT b.hash FROM blobs b \
             WHERE b.tenant_id = $1 AND b.id = $2 AND EXISTS \
             (SELECT 1 FROM messages m \
              WHERE m.tenant_id = b.tenant_id AND m.blob_id = b.id AND m.user_id = $3)",
            self.tenant.as_str(),
            id.as_str(),
            self.user.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;
        self.blobs.get(self.tenant.as_str(), &row.hash).await
    }
}
