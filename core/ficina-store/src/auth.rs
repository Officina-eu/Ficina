//! Interim bearer auth (argon2 credentials + opaque tokens), kept out of
//! `store.rs` (Law 3). Replaced by ficina-identity (OIDC) later; the
//! token → `(tenant, user)` resolution is the stable seam.

use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ring::rand::{SecureRandom, SystemRandom};
use sqlx::PgPool;

use crate::blob::hash_hex;
use crate::error::{Result, StoreError};
use crate::id::{TenantId, UserId};

/// A freshly issued bearer token and the account it authenticates.
pub struct IssuedToken {
    /// The opaque token (returned to the client once; only its hash is
    /// stored).
    pub token: String,
    /// The account's tenant.
    pub tenant: TenantId,
    /// The account's user.
    pub user: UserId,
}

/// Sets (or replaces) a user's login credentials, argon2-hashing the
/// password. `username` is the global login key.
///
/// # Errors
/// [`StoreError::Crypto`] on a hashing failure; [`StoreError::Conflict`]
/// if the username is taken.
pub async fn set_credentials(
    pool: &PgPool,
    tenant: &TenantId,
    user: &UserId,
    username: &str,
    password: &str,
) -> Result<()> {
    let hash = hash_password(password)?;
    sqlx::query!(
        "INSERT INTO credentials (user_id, tenant_id, username, password_hash) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (user_id) DO UPDATE SET username = $3, password_hash = $4",
        user.as_str(),
        tenant.as_str(),
        username,
        hash
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Verifies a username/password and issues a fresh bearer token, or
/// `None` on any mismatch (a wrong username and a wrong password are
/// indistinguishable — no user-existence oracle, in time or result).
///
/// # Errors
/// [`StoreError::Crypto`] on a secure-random failure; [`StoreError::Db`]
/// on a database failure.
pub async fn issue_token(
    pool: &PgPool,
    username: &str,
    password: &str,
) -> Result<Option<IssuedToken>> {
    let row = sqlx::query!(
        "SELECT user_id, tenant_id, password_hash FROM credentials WHERE username = $1",
        username
    )
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        // Burn a comparable argon2 cost so a missing user is not faster
        // than a wrong password (anti-enumeration).
        let salt = SaltString::generate(&mut OsRng);
        let _ = Argon2::default().hash_password(password.as_bytes(), &salt);
        return Ok(None);
    };
    if !verify_password(password, &row.password_hash) {
        return Ok(None);
    }

    let token = random_token()?;
    let token_hash = hash_hex(token.as_bytes());
    sqlx::query!(
        "INSERT INTO api_tokens (token_hash, tenant_id, user_id) VALUES ($1, $2, $3)",
        token_hash,
        row.tenant_id,
        row.user_id
    )
    .execute(pool)
    .await?;
    Ok(Some(IssuedToken {
        token,
        tenant: TenantId::new(row.tenant_id),
        user: UserId::new(row.user_id),
    }))
}

/// Resolves a bearer token to its `(tenant, user)`, honoring expiry.
/// `None` when the token is unknown or expired.
///
/// # Errors
/// [`StoreError::Db`] on a database failure.
pub async fn resolve_token(pool: &PgPool, token: &str) -> Result<Option<(TenantId, UserId)>> {
    let token_hash = hash_hex(token.as_bytes());
    let row = sqlx::query!(
        "SELECT tenant_id, user_id FROM api_tokens \
         WHERE token_hash = $1 AND (expires_at IS NULL OR expires_at > now())",
        token_hash
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| (TenantId::new(r.tenant_id), UserId::new(r.user_id))))
}

fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| StoreError::Crypto)
}

fn verify_password(password: &str, phc: &str) -> bool {
    match PasswordHash::new(phc) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

/// A 32-byte cryptographically-random bearer token, URL-safe base64.
/// Unlike ids, a weak token is a security hole, so RNG failure is a hard
/// error rather than a fallback.
fn random_token() -> Result<String> {
    let mut bytes = [0u8; 32];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| StoreError::Crypto)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
