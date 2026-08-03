//! Control-plane service state and platform-operator authentication.
//!
//! The operator is authenticated exactly like any user — the same opaque
//! bearer token resolved by `alo-identity` (ADR 0008) — and then gated on
//! the global `is_platform_admin` flag (ADR 0012). Crucially, holding an
//! operator token grants the `/control/*` governance surface only; it is never
//! a key into any tenant's mail, which stays behind the store's tenant door.

use std::sync::Arc;

use alo_identity::Identity;
use alo_store::{Store, TenantId, UserId};
use axum::http::HeaderMap;
use axum::http::header::AUTHORIZATION;

use crate::error::Problem;

/// Process-wide control-plane state.
#[derive(Clone)]
pub struct ControlState {
    /// The message store (system handle) — used only for control operations.
    pub store: Arc<Store>,
    /// The credential authority — resolves operator bearer tokens.
    pub identity: Identity,
}

/// Builds the control-plane state.
pub fn control_state(store: Arc<Store>, identity: Identity) -> ControlState {
    ControlState { store, identity }
}

/// An authenticated platform operator. Its existence is proof the bearer
/// resolved to a user carrying `is_platform_admin`.
pub struct Operator {
    /// The operator's home tenant (the reserved `_platform` system tenant).
    pub tenant: TenantId,
    /// The operator user.
    pub user: UserId,
}

/// Resolves the `Authorization: Bearer` token to an [`Operator`], or a problem.
/// A valid token that is not a platform operator gets a clean 403 — no
/// existence oracle, no partial access.
///
/// # Errors
/// [`Problem::unauthorized`] when the token is missing/invalid/revoked;
/// [`Problem::forbidden`] when the principal is not a platform operator;
/// [`Problem::server_error`] on a store failure.
pub async fn authenticate_operator(
    state: &ControlState,
    headers: &HeaderMap,
) -> Result<Operator, Problem> {
    let token = bearer_token(headers).ok_or_else(Problem::unauthorized)?;
    let principal = state
        .identity
        .resolve_access_token(&token)
        .await
        .map_err(|_| Problem::server_error())?
        .ok_or_else(Problem::unauthorized)?;
    let is_operator = state
        .store
        .user_is_platform_admin(&principal.tenant, &principal.user)
        .await
        .map_err(|_| Problem::server_error())?;
    if !is_operator {
        return Err(Problem::forbidden());
    }
    Ok(Operator {
        tenant: principal.tenant,
        user: principal.user,
    })
}

/// Record a control-plane action in the target tenant's audit log, best-effort
/// (a failed audit write never fails the action). The actor is labelled
/// `operator` — the operator is not one of the target tenant's users.
pub async fn audit(
    state: &ControlState,
    tenant: &TenantId,
    action: &str,
    target: Option<&str>,
    detail: Option<&str>,
) {
    if let Err(error) = state
        .store
        .record_audit(tenant, None, Some("operator"), action, target, detail)
        .await
    {
        tracing::warn!(%error, action, "control audit write failed");
    }
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?;
    value
        .strip_prefix("Bearer ")
        .map(|token| token.trim().to_owned())
        .filter(|token| !token.is_empty())
}
