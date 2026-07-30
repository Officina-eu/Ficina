//! Domain handlers (ADR 0012 security spine): register a domain to a tenant,
//! verify ownership by DNS TXT proof, list, and remove. A verified domain is
//! the precondition the mail services enforce before a tenant may assign an
//! address in it, closing the cross-tenant mail-capture path.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::{Json, body::Bytes};
use ficina_store::{DomainRow, StoreError, TenantId};
use hickory_resolver::TokioResolver;
use hickory_resolver::proto::rr::RData;
use serde_json::{Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::error::Problem;
use crate::state::{ControlState, audit, authenticate_operator};

/// The DNS label under a domain where the ownership token is published.
const VERIFY_PREFIX: &str = "_ficina-verify";

fn iso(dt: OffsetDateTime) -> String {
    dt.format(&Rfc3339).unwrap_or_default()
}

/// The record shape returned to the operator. `verifyRecord` is exactly what to
/// publish in DNS to prove ownership.
fn domain_json(d: &DomainRow) -> Value {
    json!({
        "domain": d.domain,
        "tenantId": d.tenant_id,
        "verified": d.verified_at.is_some(),
        "verifiedAt": d.verified_at.map(iso),
        "verifyRecord": {
            "name": format!("{VERIFY_PREFIX}.{}", d.domain),
            "type": "TXT",
            "value": d.verify_token,
        },
        "createdAt": iso(d.created_at),
    })
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn store_err(e: StoreError) -> Problem {
    match e {
        StoreError::Conflict(_) => Problem::with(StatusCode::CONFLICT, "domain already registered"),
        StoreError::NotFound => Problem::not_found(),
        _ => Problem::server_error(),
    }
}

/// `GET /control/domains` — every registered domain across the deployment.
pub async fn list_domains(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Problem> {
    authenticate_operator(&state, &headers).await?;
    let domains = state
        .store
        .list_all_domains()
        .await
        .map_err(|_| Problem::server_error())?;
    let list: Vec<Value> = domains.iter().map(domain_json).collect();
    Ok(Json(json!({ "domains": list })))
}

/// `POST /control/domains` — register `domain` to a tenant (unverified). Body
/// `{ tenantId, domain }`. Returns the DNS record to publish for verification.
pub async fn create_domain(
    State(state): State<ControlState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Problem> {
    authenticate_operator(&state, &headers).await?;
    let v: Value = serde_json::from_slice(&body).map_err(|_| Problem::not_json())?;
    let tenant_id = str_field(&v, "tenantId").ok_or_else(|| Problem::bad("tenantId required"))?;
    let domain = str_field(&v, "domain").ok_or_else(|| Problem::bad("domain required"))?;
    if !is_plausible_domain(&domain) {
        return Err(Problem::bad("invalid domain"));
    }
    let tenant = TenantId::new(tenant_id);
    if !state
        .store
        .tenant_exists(&tenant)
        .await
        .map_err(|_| Problem::server_error())?
    {
        return Err(Problem::with(StatusCode::NOT_FOUND, "tenant not found"));
    }
    let row = state
        .store
        .create_domain(&tenant, &domain)
        .await
        .map_err(store_err)?;
    tracing::info!(domain = %row.domain, tenant = %row.tenant_id, "control: domain registered");
    audit(&state, &tenant, "domain.register", Some(&row.domain), None).await;
    Ok(Json(domain_json(&row)))
}

/// `POST /control/domains/verify` — check the DNS TXT proof and, if present,
/// mark the domain verified. Body `{ domain }`. Idempotent.
pub async fn verify_domain(
    State(state): State<ControlState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Problem> {
    authenticate_operator(&state, &headers).await?;
    let v: Value = serde_json::from_slice(&body).map_err(|_| Problem::not_json())?;
    let domain = str_field(&v, "domain").ok_or_else(|| Problem::bad("domain required"))?;
    let record = state
        .store
        .domain_record(&domain)
        .await
        .map_err(|_| Problem::server_error())?
        .ok_or_else(Problem::not_found)?;

    let name = format!("{VERIFY_PREFIX}.{}", record.domain);
    let found = txt_records(&name)
        .await
        .iter()
        .any(|r| r.trim() == record.verify_token);
    if !found {
        return Ok(Json(json!({
            "domain": record.domain,
            "verified": false,
            "detail": format!("no TXT record at {name} matching the token yet"),
        })));
    }
    state
        .store
        .set_domain_verified(&record.domain)
        .await
        .map_err(store_err)?;
    tracing::info!(domain = %record.domain, "control: domain verified");
    audit(
        &state,
        &TenantId::new(record.tenant_id.clone()),
        "domain.verify",
        Some(&record.domain),
        None,
    )
    .await;
    Ok(Json(json!({ "domain": record.domain, "verified": true })))
}

/// `POST /control/domains/delete` — remove a domain registration. Body
/// `{ domain }`. (A body, not a path, because a domain carries dots.)
pub async fn delete_domain(
    State(state): State<ControlState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Problem> {
    authenticate_operator(&state, &headers).await?;
    let v: Value = serde_json::from_slice(&body).map_err(|_| Problem::not_json())?;
    let domain = str_field(&v, "domain").ok_or_else(|| Problem::bad("domain required"))?;
    // Resolve the owner before deleting so the removal is audited under the
    // right tenant (the audit rows survive — they reference the tenant, not
    // the domain).
    let owner = state
        .store
        .domain_record(&domain)
        .await
        .map_err(|_| Problem::server_error())?
        .map(|r| r.tenant_id);
    state
        .store
        .delete_domain(&domain)
        .await
        .map_err(store_err)?;
    if let Some(tenant_id) = owner {
        audit(
            &state,
            &TenantId::new(tenant_id),
            "domain.delete",
            Some(&domain),
            None,
        )
        .await;
    }
    tracing::info!(domain = %domain, "control: domain removed");
    Ok(Json(json!({ "domain": domain, "deleted": true })))
}

/// A conservative structural check on a domain name (not a DNS lookup): labels
/// of `[a-z0-9-]`, at least one dot, no leading/trailing dot or hyphen.
fn is_plausible_domain(domain: &str) -> bool {
    let d = domain.trim().to_lowercase();
    if d.len() < 3 || d.len() > 253 || !d.contains('.') {
        return false;
    }
    d.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    })
}

/// Collect the TXT strings at `name` (each record's segments joined). Returns
/// an empty vec on any resolver error — an unreachable resolver reads as
/// "not verified yet", never a false positive.
async fn txt_records(name: &str) -> Vec<String> {
    let Ok(resolver) = TokioResolver::builder_tokio().and_then(|b| b.build()) else {
        return Vec::new();
    };
    match resolver.txt_lookup(name).await {
        Ok(lookup) => lookup
            .answers()
            .iter()
            .filter_map(|r| match &r.data {
                RData::TXT(txt) => Some(
                    txt.txt_data
                        .iter()
                        .map(|seg| String::from_utf8_lossy(seg))
                        .collect::<String>(),
                ),
                _ => None,
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}
