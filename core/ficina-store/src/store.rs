//! `Store` and `TenantStore` — tenancy by construction.
//!
//! `Store` does system operations only (tenants, migrations). All mail
//! data is reachable **only** through a [`TenantStore`], obtained via
//! [`Store::for_tenant`]; every query a `TenantStore` issues carries
//! `tenant_id = $tenant` by construction, so a cross-tenant access is
//! unrepresentable in the API and returns `NotFound` in the data.

use bytes::Bytes;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::blob::{BlobStore, hash_hex};
use crate::error::{Result, StoreError};
use crate::id::BlobId;
use crate::id::{MailboxId, MessageId, TenantId, ThreadId, UserId};
use crate::message;
use crate::model::{Blob, EmailQuery, Mailbox, Message, MessageSummary, Page, SortDirection};
use crate::thread;

/// The JMAP `$seen` keyword — the one that drives the unread counter.
pub const SEEN: &str = "$seen";

/// Maximum distinct keywords per message — bounds `message_keywords`
/// growth so one message cannot force an unbounded keyword set.
const MAX_KEYWORDS: i64 = 64;
/// Maximum length of a single keyword.
const MAX_KEYWORD_LEN: usize = 128;

/// The process-wide store handle: a Postgres pool plus a blob backend.
/// Its public API exposes system operations only — nothing about
/// tenant-owned rows.
#[derive(Clone)]
pub struct Store {
    pool: PgPool,
    blobs: BlobStore,
}

impl Store {
    /// Connects a pool to `database_url` and attaches `blobs`.
    ///
    /// # Errors
    /// [`StoreError::Db`] if the pool cannot connect.
    pub async fn connect(database_url: &str, blobs: BlobStore) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(16)
            .connect(database_url)
            .await
            .map_err(StoreError::Db)?;
        Ok(Self { pool, blobs })
    }

    /// Wraps an existing pool (used by tests that share one).
    pub fn new(pool: PgPool, blobs: BlobStore) -> Self {
        Self { pool, blobs }
    }

    /// Applies pending schema migrations.
    ///
    /// # Errors
    /// [`StoreError::Migrate`] on a failed migration.
    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }

    /// Creates a tenant, returning its opaque id.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn create_tenant(&self, name: &str) -> Result<TenantId> {
        let id = TenantId::generate();
        sqlx::query!(
            "INSERT INTO tenants (id, name) VALUES ($1, $2)",
            id.as_str(),
            name
        )
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    /// Whether a tenant exists (a system lookup, not tenant data).
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn tenant_exists(&self, tenant: &TenantId) -> Result<bool> {
        let row = sqlx::query!(
            "SELECT 1 AS one FROM tenants WHERE id = $1",
            tenant.as_str()
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.is_some())
    }

    /// The **only** door to mail data: a handle scoped to one tenant.
    /// Pure — no I/O; every operation it exposes is tenant-scoped.
    pub fn for_tenant(&self, tenant: TenantId) -> TenantStore {
        TenantStore {
            pool: self.pool.clone(),
            blobs: self.blobs.clone(),
            tenant,
        }
    }

    /// Interim auth: verifies a username/password (global login key) and
    /// issues a bearer token. `None` on any mismatch. See
    /// [`crate::auth`].
    ///
    /// # Errors
    /// [`StoreError::Crypto`]/[`StoreError::Db`] on failure.
    pub async fn issue_token(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Option<crate::auth::IssuedToken>> {
        crate::auth::issue_token(&self.pool, username, password).await
    }

    /// Interim auth: resolves a bearer token to `(tenant, user)`. The
    /// tenant claim comes from here, never from a request body.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn resolve_token(&self, token: &str) -> Result<Option<(TenantId, UserId)>> {
        crate::auth::resolve_token(&self.pool, token).await
    }
}

/// A tenant-scoped handle. Holds its [`TenantId`] privately and bakes it
/// into every statement. No method accepts a tenant argument.
#[derive(Clone)]
pub struct TenantStore {
    pool: PgPool,
    blobs: BlobStore,
    tenant: TenantId,
}

impl TenantStore {
    /// The tenant this handle is scoped to.
    pub fn tenant(&self) -> &TenantId {
        &self.tenant
    }

    // ---- ownership guards ---------------------------------------------
    // Write paths that accept an id must confirm it belongs to this
    // tenant, so a caller passing another tenant's id gets a clean
    // `NotFound` instead of silently creating a cross-tenant row.

    async fn assert_user(&self, user: &UserId) -> Result<()> {
        sqlx::query!(
            "SELECT 1 AS one FROM users WHERE tenant_id = $1 AND id = $2",
            self.tenant.as_str(),
            user.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)
        .map(|_| ())
    }

    async fn assert_mailbox(&self, mailbox: &MailboxId) -> Result<()> {
        sqlx::query!(
            "SELECT 1 AS one FROM mailboxes WHERE tenant_id = $1 AND id = $2",
            self.tenant.as_str(),
            mailbox.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)
        .map(|_| ())
    }

    /// Confirms the message and the mailbox are this tenant's *and* belong
    /// to the same user — a message may only be filed into its own
    /// account's mailboxes. A foreign or cross-user pair is `NotFound`.
    async fn assert_message_mailbox_same_user(
        &self,
        message: &MessageId,
        mailbox: &MailboxId,
    ) -> Result<()> {
        sqlx::query!(
            "SELECT 1 AS one FROM messages m \
             JOIN mailboxes mb ON mb.user_id = m.user_id AND mb.tenant_id = m.tenant_id \
             WHERE m.tenant_id = $1 AND m.id = $2 AND mb.id = $3",
            self.tenant.as_str(),
            message.as_str(),
            mailbox.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)
        .map(|_| ())
    }

    /// Confirms the mailbox belongs to `user` within this tenant — a
    /// message may only be filed into its own account's mailbox.
    async fn assert_mailbox_of_user(&self, mailbox: &MailboxId, user: &UserId) -> Result<()> {
        sqlx::query!(
            "SELECT 1 AS one FROM mailboxes WHERE tenant_id = $1 AND id = $2 AND user_id = $3",
            self.tenant.as_str(),
            mailbox.as_str(),
            user.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)
        .map(|_| ())
    }

    /// Locks the message row `FOR UPDATE` inside `tx` (also a tenant-scoped
    /// existence check). All counter-affecting operations on a message
    /// take this lock so `$seen`-state and membership changes serialize —
    /// preventing a stale-delta counter drift.
    async fn lock_message(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        message: &MessageId,
    ) -> Result<String> {
        sqlx::query!(
            "SELECT user_id FROM messages WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
            self.tenant.as_str(),
            message.as_str()
        )
        .fetch_optional(&mut **tx)
        .await?
        .map(|row| row.user_id)
        .ok_or(StoreError::NotFound)
    }

    /// The owning user of a mailbox (tenant-scoped) — for change records
    /// and ownership checks.
    async fn mailbox_user(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        mailbox: &MailboxId,
    ) -> Result<String> {
        sqlx::query!(
            "SELECT user_id FROM mailboxes WHERE tenant_id = $1 AND id = $2",
            self.tenant.as_str(),
            mailbox.as_str()
        )
        .fetch_optional(&mut **tx)
        .await?
        .map(|row| row.user_id)
        .ok_or(StoreError::NotFound)
    }

    // ---- account-ownership guards (JMAP account = user) ----------------

    /// Confirms a message belongs to `user`. `NotFound` otherwise.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if absent/foreign/other-user.
    pub async fn owns_message(&self, user: &UserId, message: &MessageId) -> Result<()> {
        sqlx::query!(
            "SELECT 1 AS one FROM messages WHERE tenant_id = $1 AND user_id = $2 AND id = $3",
            self.tenant.as_str(),
            user.as_str(),
            message.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)
        .map(|_| ())
    }

    /// Confirms a mailbox belongs to `user`. `NotFound` otherwise.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if absent/foreign/other-user.
    pub async fn owns_mailbox(&self, user: &UserId, mailbox: &MailboxId) -> Result<()> {
        sqlx::query!(
            "SELECT 1 AS one FROM mailboxes WHERE tenant_id = $1 AND user_id = $2 AND id = $3",
            self.tenant.as_str(),
            user.as_str(),
            mailbox.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)
        .map(|_| ())
    }

    /// Confirms `user` has at least one message in a thread (thread
    /// membership is per-user after the user-scoped thread resolution).
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if the thread has no message owned by `user`.
    pub async fn owns_thread(&self, user: &UserId, thread: &ThreadId) -> Result<()> {
        sqlx::query!(
            "SELECT 1 AS one FROM messages \
             WHERE tenant_id = $1 AND user_id = $2 AND thread_id = $3 LIMIT 1",
            self.tenant.as_str(),
            user.as_str(),
            thread.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)
        .map(|_| ())
    }

    /// Confirms `user` has a message referencing this blob (JMAP blob
    /// access is per-account). `NotFound` otherwise.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if no owned message references the blob.
    pub async fn owns_blob(&self, user: &UserId, blob: &BlobId) -> Result<()> {
        sqlx::query!(
            "SELECT 1 AS one FROM messages \
             WHERE tenant_id = $1 AND user_id = $2 AND blob_id = $3 LIMIT 1",
            self.tenant.as_str(),
            user.as_str(),
            blob.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)
        .map(|_| ())
    }

    /// Records one account's object changes and bumps the tenant modseq
    /// within `tx`. Every change in a call belongs to `user`.
    async fn record(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user: &str,
        changes: &[crate::changes::Change<'_>],
    ) -> Result<i64> {
        crate::changes::bump_and_record(tx, self.tenant.as_str(), user, changes).await
    }

    /// Mailbox ids a message is a member of (tenant-scoped) — used to
    /// cascade change records and counter adjustments.
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

    /// The tenant's current JMAP state (modseq) as an opaque token.
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
    /// modseq), bounded by `max`.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn changes(
        &self,
        user: &UserId,
        obj_type: &str,
        since: i64,
        max: i64,
    ) -> Result<crate::Changes> {
        crate::changes::changes_since(
            &self.pool,
            self.tenant.as_str(),
            user.as_str(),
            obj_type,
            since,
            max,
        )
        .await
    }

    /// Sets a user's interim login credentials.
    ///
    /// # Errors
    /// [`StoreError::Crypto`]/[`StoreError::Db`] on failure.
    pub async fn set_credentials(
        &self,
        user: &UserId,
        username: &str,
        password: &str,
    ) -> Result<()> {
        self.assert_user(user).await?;
        crate::auth::set_credentials(&self.pool, &self.tenant, user, username, password).await
    }

    // ---- users ---------------------------------------------------------

    /// Creates a user (JMAP account) in this tenant.
    ///
    /// # Errors
    /// [`StoreError::Conflict`] if the email already exists in the tenant.
    pub async fn create_user(&self, email: &str) -> Result<UserId> {
        let id = UserId::generate();
        sqlx::query!(
            "INSERT INTO users (id, tenant_id, email) VALUES ($1, $2, $3)",
            id.as_str(),
            self.tenant.as_str(),
            email
        )
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    /// Looks up a user id by email within this tenant.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if no such user in this tenant.
    pub async fn user_by_email(&self, email: &str) -> Result<UserId> {
        let row = sqlx::query!(
            "SELECT id FROM users WHERE tenant_id = $1 AND email = $2",
            self.tenant.as_str(),
            email
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;
        Ok(UserId::new(row.id))
    }

    // ---- mailboxes -----------------------------------------------------

    /// Creates a mailbox for `user`, optionally under `parent` and with a
    /// JMAP `role`.
    ///
    /// # Errors
    /// [`StoreError::Conflict`] on a duplicate sibling name or role.
    pub async fn create_mailbox(
        &self,
        user: &UserId,
        parent: Option<&MailboxId>,
        name: &str,
        role: Option<&str>,
    ) -> Result<MailboxId> {
        self.assert_user(user).await?;
        if let Some(parent) = parent {
            self.assert_mailbox(parent).await?;
        }
        let id = MailboxId::generate();
        let mut tx = self.pool.begin().await.map_err(StoreError::Db)?;
        sqlx::query!(
            "INSERT INTO mailboxes (id, tenant_id, user_id, parent_id, name, role) \
             VALUES ($1, $2, $3, $4, $5, $6)",
            id.as_str(),
            self.tenant.as_str(),
            user.as_str(),
            parent.map(MailboxId::as_str),
            name,
            role
        )
        .execute(&mut *tx)
        .await?;
        self.record(
            &mut tx,
            user.as_str(),
            &[crate::changes::Change::created(
                crate::changes::TYPE_MAILBOX,
                id.as_str(),
            )],
        )
        .await?;
        tx.commit().await.map_err(StoreError::Db)?;
        Ok(id)
    }

    /// Gets-or-creates the user's `inbox` role mailbox.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn inbox(&self, user: &UserId) -> Result<MailboxId> {
        self.assert_user(user).await?;
        if let Some(row) = sqlx::query!(
            "SELECT id FROM mailboxes WHERE tenant_id = $1 AND user_id = $2 AND role = 'inbox'",
            self.tenant.as_str(),
            user.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        {
            return Ok(MailboxId::new(row.id));
        }
        self.create_mailbox(user, None, "Inbox", Some("inbox"))
            .await
    }

    /// Fetches a mailbox (tenant-scoped). Wrong-tenant → `NotFound`.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if absent or owned by another tenant.
    pub async fn mailbox(&self, id: &MailboxId) -> Result<Mailbox> {
        let row = sqlx::query!(
            "SELECT id, parent_id, name, role, total_messages, unread_messages \
             FROM mailboxes WHERE tenant_id = $1 AND id = $2",
            self.tenant.as_str(),
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

    /// Lists a user's mailboxes (paginated).
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn mailboxes_for_user(&self, user: &UserId, page: Page) -> Result<Vec<Mailbox>> {
        let rows = sqlx::query!(
            "SELECT id, parent_id, name, role, total_messages, unread_messages \
             FROM mailboxes WHERE tenant_id = $1 AND user_id = $2 \
             ORDER BY name LIMIT $3 OFFSET $4",
            self.tenant.as_str(),
            user.as_str(),
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

    /// Delivers a raw message into `user`'s inbox (the SMTP/migration
    /// path). Convenience over [`Self::ingest`].
    ///
    /// # Errors
    /// See [`Self::ingest`].
    pub async fn deliver(&self, user: &UserId, raw: &[u8]) -> Result<MessageId> {
        let inbox = self.inbox(user).await?;
        self.ingest(user, &inbox, raw).await
    }

    /// Ingests a raw message: content-address the bytes to the blob store
    /// (first — see crash-safety note), then in one transaction thread
    /// it, insert the row, add mailbox membership, bump counters, and
    /// build the search vector.
    ///
    /// # Errors
    /// [`StoreError::TooLarge`] over the blob ceiling; [`StoreError::Db`]
    /// / [`StoreError::Blob`] on failure.
    pub async fn ingest(
        &self,
        user: &UserId,
        mailbox: &MailboxId,
        raw: &[u8],
    ) -> Result<MessageId> {
        // Bound the size before any parse/copy/blob work.
        if raw.len() > self.blobs.max_size() {
            return Err(StoreError::TooLarge {
                size: raw.len(),
                limit: self.blobs.max_size(),
            });
        }
        // Reject a cross-tenant user, or a mailbox that is not this user's,
        // before writing any blob.
        self.assert_user(user).await?;
        self.assert_mailbox_of_user(mailbox, user).await?;

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
        let new_blob_id = crate::id::BlobId::generate();
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

        // Thread: join the thread of any earlier message THIS USER sent
        // that we reference (threads are per-account).
        let (thread_id, thread_created) = self
            .resolve_thread(&mut tx, user, &parsed.referenced_ids, &parsed.subject)
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
            user.as_str(),
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
            user.as_str(),
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
    /// thread (so ingestion can record the right change type).
    async fn resolve_thread(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        user: &UserId,
        referenced_ids: &[String],
        subject: &str,
    ) -> Result<(ThreadId, bool)> {
        if !referenced_ids.is_empty() {
            let existing = sqlx::query!(
                "SELECT thread_id FROM messages \
                 WHERE tenant_id = $1 AND user_id = $2 AND message_id_hdr = ANY($3::text[]) \
                 ORDER BY created_at LIMIT 1",
                self.tenant.as_str(),
                user.as_str(),
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
    /// by the `(tenant_id, mailbox_id, added_at DESC)` index.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn list_mailbox(
        &self,
        mailbox: &MailboxId,
        page: Page,
    ) -> Result<Vec<MessageSummary>> {
        let rows = sqlx::query!(
            "SELECT m.id, m.thread_id, m.subject, m.from_addr, m.sent_at, m.received_at, m.size \
             FROM mailbox_messages mm \
             JOIN messages m ON m.id = mm.message_id AND m.tenant_id = mm.tenant_id \
             WHERE mm.tenant_id = $1 AND mm.mailbox_id = $2 \
             ORDER BY mm.added_at DESC LIMIT $3 OFFSET $4",
            self.tenant.as_str(),
            mailbox.as_str(),
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

    /// Fetches a message's metadata (tenant-scoped).
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if absent or owned by another tenant.
    pub async fn message(&self, id: &MessageId) -> Result<Message> {
        let row = sqlx::query!(
            "SELECT id, thread_id, blob_id, message_id_hdr, subject, from_addr, to_addrs, \
             sent_at, received_at, size, auth_spf, auth_dkim, auth_dmarc \
             FROM messages WHERE tenant_id = $1 AND id = $2",
            self.tenant.as_str(),
            id.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;
        Ok(Message {
            id: MessageId::new(row.id),
            thread_id: ThreadId::new(row.thread_id),
            blob_id: crate::id::BlobId::new(row.blob_id),
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

    /// Fetches a message's raw bytes from the blob store (tenant-scoped;
    /// the blob hash is resolved via the tenant's own message row).
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if the message is absent/foreign;
    /// [`StoreError::Blob`] on a blob failure.
    pub async fn message_bytes(&self, id: &MessageId) -> Result<Bytes> {
        let row = sqlx::query!(
            "SELECT b.hash FROM messages m JOIN blobs b ON b.id = m.blob_id \
             WHERE m.tenant_id = $1 AND m.id = $2",
            self.tenant.as_str(),
            id.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;
        self.blobs.get(self.tenant.as_str(), &row.hash).await
    }

    /// The keywords set on a message (tenant-scoped).
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn keywords(&self, message: &MessageId) -> Result<Vec<String>> {
        let rows = sqlx::query!(
            "SELECT keyword FROM message_keywords WHERE tenant_id = $1 AND message_id = $2 \
             ORDER BY keyword",
            self.tenant.as_str(),
            message.as_str()
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.keyword).collect())
    }

    // ---- flags & state -------------------------------------------------

    /// Sets or clears a keyword on a message, maintaining the unread
    /// counter of every mailbox the message is in transactionally.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if the message is absent/foreign.
    pub async fn set_keyword(&self, message: &MessageId, keyword: &str, on: bool) -> Result<()> {
        if on && keyword.len() > MAX_KEYWORD_LEN {
            return Err(StoreError::Conflict("keyword too long".to_owned()));
        }
        let mut tx = self.pool.begin().await.map_err(StoreError::Db)?;

        // Lock the message row: existence check + serialization against
        // add/remove_from_mailbox so the unread delta is never stale.
        let owner = self.lock_message(&mut tx, message).await?;

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
            self.record(&mut tx, &owner, &records).await?;
        }

        tx.commit().await.map_err(StoreError::Db)?;
        Ok(())
    }

    /// Adds a message to a mailbox (idempotent), bumping counters when it
    /// was not already a member.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if the message is absent/foreign.
    pub async fn add_to_mailbox(&self, message: &MessageId, mailbox: &MailboxId) -> Result<()> {
        // Message and mailbox must be this tenant's AND the same user's.
        self.assert_message_mailbox_same_user(message, mailbox)
            .await?;
        let mut tx = self.pool.begin().await.map_err(StoreError::Db)?;
        // Lock serializes the seen-state read against set_keyword.
        let owner = self.lock_message(&mut tx, message).await?;
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
                &owner,
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

    /// Removes a message from a mailbox, adjusting counters when it was a
    /// member.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn remove_from_mailbox(
        &self,
        message: &MessageId,
        mailbox: &MailboxId,
    ) -> Result<()> {
        self.assert_message_mailbox_same_user(message, mailbox)
            .await?;
        let mut tx = self.pool.begin().await.map_err(StoreError::Db)?;
        let owner = self.lock_message(&mut tx, message).await?;
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
                &owner,
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

    // ---- search --------------------------------------------------------

    /// Full-text search over a user's messages (subject/addresses/body),
    /// tenant-scoped and paginated.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn search(
        &self,
        user: &UserId,
        query: &str,
        page: Page,
    ) -> Result<Vec<MessageSummary>> {
        let rows = sqlx::query!(
            "SELECT id, thread_id, subject, from_addr, sent_at, received_at, size \
             FROM messages \
             WHERE tenant_id = $1 AND user_id = $2 AND search @@ plainto_tsquery('simple', $3) \
             ORDER BY received_at DESC LIMIT $4 OFFSET $5",
            self.tenant.as_str(),
            user.as_str(),
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

    /// The message ids in a thread (tenant-scoped), oldest first.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn thread_messages(&self, thread: &ThreadId, page: Page) -> Result<Vec<MessageId>> {
        let rows = sqlx::query!(
            "SELECT id FROM messages WHERE tenant_id = $1 AND thread_id = $2 \
             ORDER BY created_at LIMIT $3 OFFSET $4",
            self.tenant.as_str(),
            thread.as_str(),
            page.limit(),
            page.offset()
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| MessageId::new(r.id)).collect())
    }

    // ---- mailbox mutations (Mailbox/set) -------------------------------

    /// Renames a mailbox. Records a Mailbox change.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if absent/foreign; [`StoreError::Conflict`]
    /// on a duplicate sibling name.
    pub async fn rename_mailbox(&self, id: &MailboxId, name: &str) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(StoreError::Db)?;
        let owner = self.mailbox_user(&mut tx, id).await?;
        sqlx::query!(
            "UPDATE mailboxes SET name = $3 WHERE tenant_id = $1 AND id = $2",
            self.tenant.as_str(),
            id.as_str(),
            name
        )
        .execute(&mut *tx)
        .await?;
        self.record(
            &mut tx,
            &owner,
            &[crate::changes::Change::updated(
                crate::changes::TYPE_MAILBOX,
                id.as_str(),
            )],
        )
        .await?;
        tx.commit().await.map_err(StoreError::Db)?;
        Ok(())
    }

    /// Moves a mailbox under a new parent (`None` = root). Records a
    /// Mailbox change.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if the mailbox or parent is absent/foreign;
    /// [`StoreError::Conflict`] if the move would create a cycle or clash.
    pub async fn move_mailbox(&self, id: &MailboxId, parent: Option<&MailboxId>) -> Result<()> {
        if let Some(parent) = parent {
            self.assert_mailbox(parent).await?;
            if parent == id {
                return Err(StoreError::Conflict(
                    "mailbox cannot parent itself".to_owned(),
                ));
            }
        }
        let mut tx = self.pool.begin().await.map_err(StoreError::Db)?;
        let owner = self.mailbox_user(&mut tx, id).await?;
        sqlx::query!(
            "UPDATE mailboxes SET parent_id = $3 WHERE tenant_id = $1 AND id = $2",
            self.tenant.as_str(),
            id.as_str(),
            parent.map(MailboxId::as_str)
        )
        .execute(&mut *tx)
        .await?;
        self.record(
            &mut tx,
            &owner,
            &[crate::changes::Change::updated(
                crate::changes::TYPE_MAILBOX,
                id.as_str(),
            )],
        )
        .await?;
        tx.commit().await.map_err(StoreError::Db)?;
        Ok(())
    }

    /// Destroys an empty, childless mailbox. Records a Mailbox tombstone.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if absent/foreign; [`StoreError::Conflict`]
    /// (mapped to JMAP `mailboxHasEmail`/`mailboxHasChild`) if it still
    /// holds messages or sub-mailboxes.
    pub async fn destroy_mailbox(&self, id: &MailboxId) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(StoreError::Db)?;
        let owner = self.mailbox_user(&mut tx, id).await?;
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
            "DELETE FROM mailboxes WHERE tenant_id = $1 AND id = $2",
            self.tenant.as_str(),
            id.as_str()
        )
        .execute(&mut *tx)
        .await?;
        self.record(
            &mut tx,
            &owner,
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

    /// The mailbox ids a message belongs to (tenant-scoped) — for
    /// `Email/get` `mailboxIds`.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if the message is absent/foreign.
    pub async fn mailboxes_of_message(&self, message: &MessageId) -> Result<Vec<MailboxId>> {
        self.assert_message_exists(message).await?;
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

    async fn assert_message_exists(&self, message: &MessageId) -> Result<()> {
        sqlx::query!(
            "SELECT 1 AS one FROM messages WHERE tenant_id = $1 AND id = $2",
            self.tenant.as_str(),
            message.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)
        .map(|_| ())
    }

    /// `Email/query`: filters + `receivedAt` sort + bounded page.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn query_emails(&self, user: &UserId, q: &EmailQuery) -> Result<Vec<MessageSummary>> {
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
                user.as_str(),
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
                user.as_str(),
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

    /// Destroys a message everywhere: adjusts every containing mailbox's
    /// counters, deletes the row (membership/keywords cascade), and
    /// records the Email tombstone plus the affected Mailbox updates.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if absent/foreign.
    pub async fn destroy_message(&self, message: &MessageId) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(StoreError::Db)?;
        let owner = self.lock_message(&mut tx, message).await?;
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
            "DELETE FROM messages WHERE tenant_id = $1 AND id = $2",
            self.tenant.as_str(),
            message.as_str()
        )
        .execute(&mut *tx)
        .await?;
        use crate::changes::{Change, TYPE_EMAIL, TYPE_MAILBOX};
        let mut records = vec![Change::destroyed(TYPE_EMAIL, message.as_str())];
        for mb in &mailbox_ids {
            records.push(Change::updated(TYPE_MAILBOX, mb));
        }
        self.record(&mut tx, &owner, &records).await?;
        tx.commit().await.map_err(StoreError::Db)?;
        Ok(())
    }

    // ---- blobs (JMAP upload/download) ---------------------------------

    /// Stores an uploaded blob (content-addressed) and returns its id.
    /// Idempotent for identical content.
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

    /// A blob's metadata (tenant-scoped).
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if absent/foreign.
    pub async fn blob(&self, id: &BlobId) -> Result<Blob> {
        let row = sqlx::query!(
            "SELECT id, size, content_type FROM blobs WHERE tenant_id = $1 AND id = $2",
            self.tenant.as_str(),
            id.as_str()
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

    /// A blob's bytes (tenant-scoped; read only under this tenant's
    /// prefix).
    ///
    /// # Errors
    /// [`StoreError::NotFound`] if absent/foreign; [`StoreError::Blob`] on
    /// a blob-store failure.
    pub async fn blob_bytes(&self, id: &BlobId) -> Result<Bytes> {
        let row = sqlx::query!(
            "SELECT hash FROM blobs WHERE tenant_id = $1 AND id = $2",
            self.tenant.as_str(),
            id.as_str()
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;
        self.blobs.get(self.tenant.as_str(), &row.hash).await
    }
}
