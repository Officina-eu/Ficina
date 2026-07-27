//! `Store`, `TenantStore` — tenancy by construction; the account door.
//!
//! `Store` does system operations only (tenants, migrations, auth).
//! **Tenant-level** operations — user provisioning and lookup — go
//! through a [`TenantStore`] ([`Store::for_tenant`]). **User-owned mail
//! data** (mailboxes, messages, threads, keywords, blobs, the change
//! log) is reachable only through an [`AccountStore`](crate::AccountStore)
//! ([`Store::for_account`]), which bakes `(tenant, user)` into every
//! statement so cross-account access is unrepresentable (see
//! `docs/design/account-scoped-access-door.md`).

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

use crate::blob::BlobStore;
use crate::error::{Result, StoreError};
use crate::id::{TenantId, UserId};

/// The JMAP `$seen` keyword — the one that drives the unread counter.
pub const SEEN: &str = "$seen";

/// Maximum distinct keywords per message — bounds `message_keywords`
/// growth so one message cannot force an unbounded keyword set.
pub(crate) const MAX_KEYWORDS: i64 = 64;
/// Maximum length of a single keyword.
pub(crate) const MAX_KEYWORD_LEN: usize = 128;

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

    /// A tenant-scoped handle for genuinely tenant-level operations
    /// (user provisioning and lookup). Pure — no I/O.
    pub fn for_tenant(&self, tenant: TenantId) -> TenantStore {
        TenantStore {
            pool: self.pool.clone(),
            tenant,
        }
    }

    /// The **only** door to user-owned mail data: a handle scoped to one
    /// `(tenant, user)`. Pure — no I/O; every operation it exposes bakes
    /// both ids, so cross-account access is unrepresentable (see
    /// [`crate::account::AccountStore`]).
    pub fn for_account(&self, tenant: TenantId, user: UserId) -> crate::account::AccountStore {
        crate::account::AccountStore {
            pool: self.pool.clone(),
            blobs: self.blobs.clone(),
            tenant,
            user,
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

    /// Interim auth for stateful protocols (IMAP/POP3 `LOGIN`): verifies a
    /// username/password and resolves it to `(tenant, user)` without
    /// issuing a bearer token. `None` on any mismatch. See [`crate::auth`].
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn verify_login(
        &self,
        username: &str,
        password: &str,
    ) -> Result<Option<(TenantId, UserId)>> {
        crate::auth::verify_login(&self.pool, username, password).await
    }
}

/// A tenant-scoped handle for tenant-level provisioning. Holds its
/// [`TenantId`] privately and bakes it into every statement. No method
/// accepts a tenant argument. User-owned mail data is **not** reachable
/// here — that is [`AccountStore`](crate::AccountStore)'s job.
#[derive(Clone)]
pub struct TenantStore {
    pool: PgPool,
    tenant: TenantId,
}

impl TenantStore {
    /// The tenant this handle is scoped to.
    pub fn tenant(&self) -> &TenantId {
        &self.tenant
    }

    /// Confirms a user exists in this tenant; `NotFound` otherwise. Guards
    /// the provisioning paths that take a user id.
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

    // ---- users (tenant-level provisioning) ----------------------------

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
}
