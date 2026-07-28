//! Shared service state, honest limits, and bearer authentication.

use std::sync::Arc;

use axum::http::HeaderMap;
use axum::http::header::AUTHORIZATION;
use ficina_identity::Identity;
use ficina_store::{AccountStore, Store, TenantId, UserId};

use crate::error::Problem;
use crate::push::PushHub;

/// Process-wide JMAP service state.
#[derive(Clone)]
pub struct AppState {
    /// The message store (system handle).
    pub store: Arc<Store>,
    /// The credential authority — resolves bearer tokens to accounts.
    pub identity: Identity,
    /// Per-tenant push fan-out for EventSource.
    pub push: PushHub,
    /// Advertised, enforced limits.
    pub limits: Limits,
    /// Externally-visible base URL, for building session URLs.
    pub base_url: String,
}

/// The limits advertised in the Session resource and enforced on every
/// request. Real values, documented in `docs/design/jmap-api.md`.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// `maxSizeUpload` (octets).
    pub max_size_upload: u64,
    /// `maxSizeRequestObject` (octets) — bounded before parse.
    pub max_size_request: usize,
    /// `maxConcurrentUpload`.
    pub max_concurrent_upload: u64,
    /// `maxCallsInRequest`.
    pub max_calls_in_request: usize,
    /// `maxObjectsInGet`.
    pub max_objects_in_get: usize,
    /// `maxObjectsInSet`.
    pub max_objects_in_set: usize,
    /// Truncation ceiling for `Email/get` `bodyValues`.
    pub max_body_value_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_size_upload: 50 * 1024 * 1024,
            max_size_request: 10 * 1024 * 1024,
            max_concurrent_upload: 4,
            max_calls_in_request: 32,
            max_objects_in_get: 500,
            max_objects_in_set: 500,
            max_body_value_bytes: 256 * 1024,
        }
    }
}

/// An authenticated account: the resolved tenant/user and the store door
/// scoped to that `(tenant, user)`. Obtained only via [`authenticate`].
/// The door bakes both ids, so every store call is account-scoped by
/// construction — no ownership guard to remember.
pub struct Account {
    /// The tenant claim (from the token, never the request body).
    pub tenant: TenantId,
    /// The account's user.
    pub user: UserId,
    /// The account-scoped store handle — the only path to this user's
    /// mail data.
    pub acc: AccountStore,
}

impl Account {
    /// The JMAP accountId (the user id).
    pub fn account_id(&self) -> &str {
        self.user.as_str()
    }
}

/// Resolves the `Authorization: Bearer` token to an [`Account`] via
/// `ficina-identity`. The tenant is taken from the token, never the
/// request. A revoked or expired token resolves to `unauthorized`.
///
/// # Errors
/// [`Problem::unauthorized`] when the token is missing/invalid/revoked;
/// [`Problem::server_error`] on a store failure.
pub async fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<Account, Problem> {
    let token = bearer_token(headers).ok_or_else(Problem::unauthorized)?;
    let principal = state
        .identity
        .resolve_access_token(&token)
        .await
        .map_err(|_| Problem::server_error())?
        .ok_or_else(Problem::unauthorized)?;
    let acc = state
        .store
        .for_account(principal.tenant.clone(), principal.user.clone());
    Ok(Account {
        tenant: principal.tenant,
        user: principal.user,
        acc,
    })
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?;
    value
        .strip_prefix("Bearer ")
        .map(|token| token.trim().to_owned())
        .filter(|token| !token.is_empty())
}
