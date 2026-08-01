//! Shared service state, honest limits, and bearer authentication.

use std::sync::Arc;

use alo_identity::Identity;
use alo_store::{AccountStore, Store, TenantId, UserId};
use axum::http::HeaderMap;
use axum::http::header::AUTHORIZATION;

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
    /// `host:port` of the SMTP trusted internal submission listener, used by
    /// `EmailSubmission/set` to send. `None` disables sending (the capability
    /// is still advertised but a submit returns `forbiddenToSend`).
    pub submission_addr: Option<String>,
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
    /// Whether this user is a tenant admin (gates admin-only surfaces).
    pub is_admin: bool,
    /// Delegation status of THIS account handle (ADR 0017). `None` when it is
    /// the signed-in user's own account (full rights). `Some(can_send)` when it
    /// is another user's mailbox the signed-in user was granted access to — the
    /// bool is whether they may also send as that address. A delegated handle
    /// never confers admin, and `is_admin` is forced false for it.
    pub delegated: Option<bool>,
}

impl Account {
    /// The JMAP accountId (the user id).
    pub fn account_id(&self) -> &str {
        self.user.as_str()
    }

    /// Guard for admin-only endpoints.
    ///
    /// # Errors
    /// [`Problem`] 403 when the user is not a tenant admin.
    pub fn require_admin(&self) -> Result<(), Problem> {
        if self.is_admin {
            Ok(())
        } else {
            Err(Problem::with(
                axum::http::StatusCode::FORBIDDEN,
                "admin only",
            ))
        }
    }
}

/// Resolves the `Authorization: Bearer` token to an [`Account`] via
/// `alo-identity`. The tenant is taken from the token, never the
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
    let is_admin = acc.is_admin().await.unwrap_or(false);
    Ok(Account {
        tenant: principal.tenant,
        user: principal.user,
        acc,
        is_admin,
        delegated: None,
    })
}

/// Resolves the account a request targets (its `accountId`) into an [`Account`]
/// handle the signed-in user is authorized to operate on, or `None` when they
/// are not — which the caller renders as the same `accountNotFound` as any
/// unknown id (no oracle for "exists but you can't touch it").
///
/// - the signed-in user's own id → their own account (full rights);
/// - another user's id they hold a delegation grant on (same tenant) → that
///   user's mailbox as a delegated handle (`is_admin` forced false);
/// - anything else → `None`.
pub async fn resolve_target(
    signed_in: &Account,
    state: &AppState,
    account_id: &str,
) -> Option<Account> {
    if account_id == signed_in.user.as_str() {
        return Some(Account {
            tenant: signed_in.tenant.clone(),
            user: signed_in.user.clone(),
            acc: state
                .store
                .for_account(signed_in.tenant.clone(), signed_in.user.clone()),
            is_admin: signed_in.is_admin,
            delegated: None,
        });
    }
    // A mailbox the signed-in user was delegated access to. The grant is looked
    // up only within the signed-in user's own tenant, so it can never authorize
    // across tenants.
    let owner = UserId::new(account_id);
    let can_send = state
        .store
        .for_tenant(signed_in.tenant.clone())
        .delegation(&owner, &signed_in.user)
        .await
        .ok()
        .flatten()?;
    Some(Account {
        tenant: signed_in.tenant.clone(),
        acc: state.store.for_account(signed_in.tenant.clone(), owner.clone()),
        user: owner,
        is_admin: false,
        delegated: Some(can_send),
    })
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?;
    value
        .strip_prefix("Bearer ")
        .map(|token| token.trim().to_owned())
        .filter(|token| !token.is_empty())
}
