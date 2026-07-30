//! Tenant lifecycle handlers (ADR 0012): list, create (provision a tenant and
//! its first admin), suspend/resume, and delete. Every handler authenticates a
//! platform operator first; the operator's own tenant is irrelevant to the
//! target, which is named explicitly in the path/body.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::{Json, body::Bytes};
use ficina_store::{StoreError, TenantId, TenantSummary};
use serde_json::{Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::error::Problem;
use crate::state::{ControlState, authenticate_operator};

/// The minimum length of a bootstrapped tenant-admin password (matches the
/// `identityctl` bootstrap strength).
const MIN_ADMIN_PASSWORD: usize = 12;

fn iso(dt: OffsetDateTime) -> String {
    dt.format(&Rfc3339).unwrap_or_default()
}

fn summary_json(t: &TenantSummary) -> Value {
    json!({
        "id": t.id,
        "name": t.name,
        "status": t.status,
        "createdAt": iso(t.created_at),
        "userCount": t.user_count,
        "storageBytes": t.storage_bytes,
    })
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// Map a store error to a client problem: conflict → 409, not-found → 404,
/// everything else a 500 with no leaked internal detail.
fn store_err(e: StoreError) -> Problem {
    match e {
        StoreError::Conflict(_) => Problem::with(StatusCode::CONFLICT, "already exists"),
        StoreError::NotFound => Problem::not_found(),
        _ => Problem::server_error(),
    }
}

/// `GET /control/me` — confirm the caller is a platform operator.
pub async fn whoami(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Problem> {
    let op = authenticate_operator(&state, &headers).await?;
    Ok(Json(
        json!({ "isOperator": true, "userId": op.user.as_str() }),
    ))
}

/// `GET /control/tenants` — list every tenant with usage and lifecycle status.
pub async fn list_tenants(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Problem> {
    authenticate_operator(&state, &headers).await?;
    let tenants = state
        .store
        .list_tenants()
        .await
        .map_err(|_| Problem::server_error())?;
    let list: Vec<Value> = tenants.iter().map(summary_json).collect();
    Ok(Json(json!({ "tenants": list })))
}

/// `POST /control/tenants` — create a tenant and its first admin. Body
/// `{ name, adminEmail, adminPassword }`. Provisions the tenant, the admin
/// user with an inbox, and the admin login — the operation `identityctl
/// bootstrap-admin` does, as an audited API.
pub async fn create_tenant(
    State(state): State<ControlState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Problem> {
    authenticate_operator(&state, &headers).await?;
    let v: Value = serde_json::from_slice(&body).map_err(|_| Problem::not_json())?;
    let name = str_field(&v, "name").ok_or_else(|| Problem::bad("name required"))?;
    let email = str_field(&v, "adminEmail").ok_or_else(|| Problem::bad("adminEmail required"))?;
    let password = v.get("adminPassword").and_then(Value::as_str).unwrap_or("");
    if !email.contains('@') {
        return Err(Problem::bad("valid adminEmail required"));
    }
    if password.len() < MIN_ADMIN_PASSWORD {
        return Err(Problem::bad("adminPassword must be at least 12 characters"));
    }
    let account = state
        .identity
        .bootstrap_admin(&name, &email, password)
        .await
        .map_err(|e| match e {
            ficina_identity::IdentityError::Store(se) => store_err(se),
            _ => Problem::server_error(),
        })?;
    tracing::info!(tenant = %account.tenant.as_str(), "control: tenant provisioned");
    Ok(Json(json!({
        "id": account.tenant.as_str(),
        "adminUserId": account.user.as_str(),
    })))
}

/// `POST /control/tenants/{id}/status` — set lifecycle status. Body
/// `{ status: "active" | "suspended" }`. Suspending denies the tenant's logins
/// and defers its inbound mail; it is reversible and touches no data.
pub async fn set_status(
    State(state): State<ControlState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Problem> {
    authenticate_operator(&state, &headers).await?;
    let v: Value = serde_json::from_slice(&body).map_err(|_| Problem::not_json())?;
    let status = str_field(&v, "status").ok_or_else(|| Problem::bad("status required"))?;
    if status != "active" && status != "suspended" {
        return Err(Problem::bad("status must be active or suspended"));
    }
    let tenant = TenantId::new(id);
    state
        .store
        .set_tenant_status(&tenant, &status)
        .await
        .map_err(store_err)?;
    tracing::info!(tenant = %tenant.as_str(), status, "control: tenant status changed");
    Ok(Json(json!({ "id": tenant.as_str(), "status": status })))
}

/// `DELETE /control/tenants/{id}` — permanently delete a tenant and all its
/// data (cascade). Requires body `{ confirm: "<tenant-id>" }` echoing the id,
/// so a delete cannot fire from a mistargeted request. Always audited.
pub async fn delete_tenant(
    State(state): State<ControlState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Problem> {
    let op = authenticate_operator(&state, &headers).await?;
    let v: Value = serde_json::from_slice(&body).map_err(|_| Problem::not_json())?;
    let confirm = str_field(&v, "confirm").unwrap_or_default();
    if confirm != id {
        return Err(Problem::bad("confirm must echo the tenant id"));
    }
    // An operator cannot delete the system tenant they live in.
    if op.tenant.as_str() == id {
        return Err(Problem::with(
            StatusCode::CONFLICT,
            "cannot delete the platform system tenant",
        ));
    }
    let tenant = TenantId::new(id);
    state
        .store
        .delete_tenant(&tenant)
        .await
        .map_err(store_err)?;
    tracing::warn!(tenant = %tenant.as_str(), "control: tenant DELETED");
    Ok(Json(json!({ "id": tenant.as_str(), "deleted": true })))
}
