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
use ficina_store::AiProviderRow;
use serde_json::{Value, json};

use crate::error::Problem;
use crate::state::{AppState, authenticate};

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
