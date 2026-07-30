//! Mail settings endpoints: the signed-in user's signature (read + write) and
//! the tenant's organization footer (read for everyone, write for admins). Both
//! are HTML fragments the compose surface inserts into outgoing mail.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::{Json, body::Bytes};
use serde_json::{Value, json};

use crate::error::Problem;
use crate::state::{AppState, authenticate};

/// `GET /settings/mail` → `{ signature, orgFooter }` — the caller's own
/// signature plus the tenant footer, both for the compose surface.
pub async fn mail_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Problem> {
    let account = authenticate(&state, &headers).await?;
    let signature = account
        .acc
        .signature()
        .await
        .map_err(|_| Problem::server_error())?;
    let org_footer = state
        .store
        .org_footer(&account.tenant)
        .await
        .map_err(|_| Problem::server_error())?;
    Ok(Json(
        json!({ "signature": signature, "orgFooter": org_footer }),
    ))
}

/// `POST /settings/signature` — set the caller's signature. Body
/// `{ signature }` (HTML; empty clears it).
pub async fn set_signature(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Problem> {
    let account = authenticate(&state, &headers).await?;
    let v: Value = serde_json::from_slice(&body).map_err(|_| Problem::not_json())?;
    let signature = v.get("signature").and_then(Value::as_str).unwrap_or("");
    account
        .acc
        .set_signature(signature)
        .await
        .map_err(|_| Problem::server_error())?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /admin/org-footer` — set the tenant's organization footer (admin
/// only). Body `{ footer }` (HTML; empty clears it).
pub async fn set_org_footer(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Problem> {
    let account = authenticate(&state, &headers).await?;
    account.require_admin()?;
    let v: Value = serde_json::from_slice(&body).map_err(|_| Problem::not_json())?;
    let footer = v.get("footer").and_then(Value::as_str).unwrap_or("");
    state
        .store
        .set_org_footer(&account.tenant, footer)
        .await
        .map_err(|_| Problem::server_error())?;
    Ok(Json(json!({ "ok": true })))
}
