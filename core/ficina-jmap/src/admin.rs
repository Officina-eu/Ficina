//! Admin console endpoints (tenant-admin only). The first surface is AI
//! provider management (ADR 0011, extended): configure OpenAI-compatible
//! backends (self-hosted Ollama, OpenAI, a custom endpoint), pick the default,
//! and test connectivity. Every handler gates on `Account::require_admin`.
//!
//! Secrets never leave the server: a provider's API key is stored but only its
//! presence (`hasKey`) is returned, and it is never logged.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::{Json, body::Bytes};
use ficina_store::{AiProviderRow, StoreError, UserId};
use serde_json::{Value, json};

use crate::error::Problem;
use crate::jtypes::utc_date;
use crate::state::{AppState, authenticate};

/// Map a store error to a client problem (admin writes): conflicts (e.g. a
/// duplicate email) are 409, everything else a 500 with no leaked detail.
fn store_admin_err(e: StoreError) -> Problem {
    match e {
        StoreError::Conflict(_) => Problem::with(StatusCode::CONFLICT, "already exists"),
        StoreError::NotFound => Problem::not_found(),
        _ => Problem::server_error(),
    }
}

/// A stored provider as JSON — the API key is reduced to `hasKey`.
fn provider_json(p: &AiProviderRow) -> Value {
    json!({
        "id": p.id,
        "kind": p.kind,
        "label": p.label,
        "baseUrl": p.base_url,
        "model": p.model,
        "enabled": p.enabled,
        "isDefault": p.is_default,
        "hasKey": p.api_key.as_ref().is_some_and(|k| !k.is_empty()),
    })
}

/// `GET /admin/ai/providers` → `{ "providers": [...] }` (keys redacted).
pub async fn list_providers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Problem> {
    let account = authenticate(&state, &headers).await?;
    account.require_admin()?;
    let providers = account
        .acc
        .list_ai_providers()
        .await
        .map_err(|_| Problem::server_error())?;
    let list: Vec<Value> = providers.iter().map(provider_json).collect();
    Ok(Json(json!({ "providers": list })))
}

/// `POST /admin/ai/providers` — create or update one provider. Body:
/// `{ id, kind, label, baseUrl, model, enabled, apiKey? }`. A `null`/absent
/// `apiKey` on update keeps the stored key. `id` is client-supplied (a UUID for
/// new providers); the store's tenant guard makes a foreign id a no-op.
pub async fn upsert_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Problem> {
    let account = authenticate(&state, &headers).await?;
    account.require_admin()?;
    let v: Value = serde_json::from_slice(&body).map_err(|_| Problem::not_json())?;

    let id = str_field(&v, "id").ok_or_else(|| bad("id required"))?;
    let kind = str_field(&v, "kind").ok_or_else(|| bad("kind required"))?;
    let label = str_field(&v, "label").unwrap_or_default();
    let base_url = str_field(&v, "baseUrl").unwrap_or_default();
    let model = str_field(&v, "model").unwrap_or_default();
    let enabled = v.get("enabled").and_then(Value::as_bool).unwrap_or(false);
    // Only overwrite the key when a non-empty one is supplied.
    let api_key = v
        .get("apiKey")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty());

    account
        .acc
        .upsert_ai_provider(&id, &kind, &label, &base_url, &model, api_key, enabled)
        .await
        .map_err(|_| Problem::server_error())?;
    Ok(Json(json!({ "id": id })))
}

/// `POST /admin/ai/providers/default` — make one provider the tenant default.
/// Body: `{ id }`.
pub async fn set_default(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Problem> {
    let account = authenticate(&state, &headers).await?;
    account.require_admin()?;
    let v: Value = serde_json::from_slice(&body).map_err(|_| Problem::not_json())?;
    let id = str_field(&v, "id").ok_or_else(|| bad("id required"))?;
    account
        .acc
        .set_default_ai_provider(&id)
        .await
        .map_err(|_| Problem::server_error())?;
    Ok(Json(json!({ "id": id })))
}

/// `DELETE /admin/ai/providers/{id}` — remove a provider.
pub async fn delete_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, Problem> {
    let account = authenticate(&state, &headers).await?;
    account.require_admin()?;
    account
        .acc
        .delete_ai_provider(&id)
        .await
        .map_err(|_| Problem::server_error())?;
    Ok(Json(json!({ "id": id })))
}

/// `POST /admin/ai/test` — test connectivity to a backend without saving it.
/// Body: `{ baseUrl, apiKey? }` → `{ ok, models }` or a 502/400.
pub async fn test_connection(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Problem> {
    let account = authenticate(&state, &headers).await?;
    account.require_admin()?;
    let v: Value = serde_json::from_slice(&body).map_err(|_| Problem::not_json())?;
    let base_url = str_field(&v, "baseUrl").ok_or_else(|| bad("baseUrl required"))?;
    let api_key = v.get("apiKey").and_then(Value::as_str);
    match ficina_ai::check(&base_url, api_key).await {
        Ok(models) => Ok(Json(json!({ "ok": true, "models": models }))),
        Err(_) => Err(Problem::with(StatusCode::BAD_GATEWAY, "ai-backend")),
    }
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn bad(detail: &'static str) -> Problem {
    Problem::with(StatusCode::BAD_REQUEST, detail)
}

// ---- users & mailboxes -------------------------------------------------

const MIN_PASSWORD: usize = 8;

/// `GET /admin/users` → `{ users: [...] }` with per-user usage and aliases.
pub async fn list_users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Problem> {
    let account = authenticate(&state, &headers).await?;
    account.require_admin()?;
    let ts = state.store.for_tenant(account.tenant.clone());
    let users = ts.list_users().await.map_err(|_| Problem::server_error())?;
    let mut list = Vec::with_capacity(users.len());
    for u in &users {
        let aliases = ts
            .aliases_of(&UserId::new(u.id.clone()))
            .await
            .unwrap_or_default();
        list.push(json!({
            "id": u.id,
            "email": u.email,
            "isAdmin": u.is_admin,
            "createdAt": utc_date(u.created_at),
            "messageCount": u.message_count,
            "storageBytes": u.storage_bytes,
            "aliases": aliases,
        }));
    }
    Ok(Json(json!({ "users": list })))
}

/// `POST /admin/users` — create a user. Body `{ email, password }`. The new
/// user gets an inbox so they can receive mail immediately.
pub async fn create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Problem> {
    let account = authenticate(&state, &headers).await?;
    account.require_admin()?;
    let v: Value = serde_json::from_slice(&body).map_err(|_| Problem::not_json())?;
    let email = str_field(&v, "email").ok_or_else(|| bad("email required"))?;
    let password = v.get("password").and_then(Value::as_str).unwrap_or("");
    if !email.contains('@') {
        return Err(bad("valid email required"));
    }
    if password.len() < MIN_PASSWORD {
        return Err(bad("password too short"));
    }
    let ts = state.store.for_tenant(account.tenant.clone());
    let user = ts.create_user(&email).await.map_err(store_admin_err)?;
    state
        .identity
        .set_password(&account.tenant, &user, &email, password)
        .await
        .map_err(|_| Problem::server_error())?;
    state
        .store
        .for_account(account.tenant.clone(), user.clone())
        .inbox()
        .await
        .map_err(|_| Problem::server_error())?;
    Ok(Json(json!({ "id": user.as_str() })))
}

/// `POST /admin/users/password` — reset a user's password. Body
/// `{ userId, password }`.
pub async fn reset_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Problem> {
    let account = authenticate(&state, &headers).await?;
    account.require_admin()?;
    let v: Value = serde_json::from_slice(&body).map_err(|_| Problem::not_json())?;
    let user_id = str_field(&v, "userId").ok_or_else(|| bad("userId required"))?;
    let password = v.get("password").and_then(Value::as_str).unwrap_or("");
    if password.len() < MIN_PASSWORD {
        return Err(bad("password too short"));
    }
    let ts = state.store.for_tenant(account.tenant.clone());
    let user = UserId::new(user_id);
    let email = ts
        .email_of(&user)
        .await
        .map_err(|_| Problem::server_error())?
        .ok_or_else(Problem::not_found)?;
    state
        .identity
        .set_password(&account.tenant, &user, &email, password)
        .await
        .map_err(|_| Problem::server_error())?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /admin/users/admin` — set/clear a user's admin flag. Body
/// `{ userId, isAdmin }`. An admin may not remove their own admin (self-lockout).
pub async fn set_user_admin(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Problem> {
    let account = authenticate(&state, &headers).await?;
    account.require_admin()?;
    let v: Value = serde_json::from_slice(&body).map_err(|_| Problem::not_json())?;
    let user_id = str_field(&v, "userId").ok_or_else(|| bad("userId required"))?;
    let is_admin = v.get("isAdmin").and_then(Value::as_bool).unwrap_or(false);
    if user_id == account.user.as_str() && !is_admin {
        return Err(Problem::with(
            StatusCode::CONFLICT,
            "cannot remove your own admin",
        ));
    }
    state
        .store
        .for_tenant(account.tenant.clone())
        .set_admin(&UserId::new(user_id), is_admin)
        .await
        .map_err(|_| Problem::server_error())?;
    Ok(Json(json!({ "ok": true })))
}

/// `DELETE /admin/users/{id}` — delete a user. An admin cannot delete themself.
pub async fn delete_user(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, Problem> {
    let account = authenticate(&state, &headers).await?;
    account.require_admin()?;
    if id == account.user.as_str() {
        return Err(Problem::with(
            StatusCode::CONFLICT,
            "cannot delete yourself",
        ));
    }
    state
        .store
        .for_tenant(account.tenant.clone())
        .delete_user(&UserId::new(id))
        .await
        .map_err(|_| Problem::server_error())?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /admin/users/alias` — add an alias to a user. Body `{ userId, address }`.
pub async fn add_alias(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Problem> {
    let account = authenticate(&state, &headers).await?;
    account.require_admin()?;
    let v: Value = serde_json::from_slice(&body).map_err(|_| Problem::not_json())?;
    let user_id = str_field(&v, "userId").ok_or_else(|| bad("userId required"))?;
    let address = str_field(&v, "address").ok_or_else(|| bad("address required"))?;
    if !address.contains('@') {
        return Err(bad("valid address required"));
    }
    state
        .store
        .for_tenant(account.tenant.clone())
        .add_alias(&UserId::new(user_id), &address)
        .await
        .map_err(store_admin_err)?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /admin/users/alias/remove` — remove an alias. Body `{ address }`.
pub async fn remove_alias(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Problem> {
    let account = authenticate(&state, &headers).await?;
    account.require_admin()?;
    let v: Value = serde_json::from_slice(&body).map_err(|_| Problem::not_json())?;
    let address = str_field(&v, "address").ok_or_else(|| bad("address required"))?;
    state
        .store
        .for_tenant(account.tenant.clone())
        .remove_alias(&address)
        .await
        .map_err(|_| Problem::server_error())?;
    Ok(Json(json!({ "ok": true })))
}
