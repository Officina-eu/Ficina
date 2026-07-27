//! JMAP test harness: an in-process router over a real Postgres store,
//! per test, with a logged-in account. Requests are driven through the
//! router as a `tower::Service` (no socket).
#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use ficina_store::{AccountStore, BlobStore, Store, TenantStore, UserId};
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

pub fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://ficina:ficina-dev-only@127.0.0.1:5433/ficina".to_owned())
}

pub struct Harness {
    pub app: Router,
    pub token: String,
    pub account_id: String,
    pub email: String,
    pub store: Arc<Store>,
    /// Tenant-level door (user admin/credentials).
    pub ts: TenantStore,
    /// Account-scoped door (this user's mail).
    pub acct: AccountStore,
    pub user: UserId,
}

/// A fresh tenant + logged-in user over the shared Postgres, with the
/// JMAP router wired up.
pub async fn harness(tag: &str) -> Harness {
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url())
        .await
        .expect("connect to test postgres");
    let store = Arc::new(Store::new(pool, BlobStore::in_memory(50 * 1024 * 1024)));
    store.migrate().await.unwrap();
    let tenant = store.create_tenant(&format!("jmap-{tag}")).await.unwrap();
    // The username has a global unique index; include the random tenant id
    // so reruns against the shared database never collide.
    let email = format!("{tag}-{tenant}@example.test");
    let ts = store.for_tenant(tenant.clone());
    let user = ts.create_user(&email).await.unwrap();
    ts.set_credentials(&user, &email, "s3cret-pw")
        .await
        .unwrap();
    let acct = store.for_account(tenant, user.clone());
    let token = store
        .issue_token(&email, "s3cret-pw")
        .await
        .unwrap()
        .expect("token issued")
        .token;
    let app = ficina_jmap::app(ficina_jmap::app_state(Arc::clone(&store), "http://test"));
    Harness {
        app,
        token,
        account_id: user.to_string(),
        email,
        store,
        ts,
        acct,
        user,
    }
}

/// Sends a raw request through the router; returns (status, body-json).
pub async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024 * 1024)
        .await
        .unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

/// POSTs a JMAP Request to `/jmap/api` with the given bearer token.
pub async fn api(app: &Router, token: &str, body: Value) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri("/jmap/api")
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    send(app, req).await
}

/// A single method call wrapped in a Request envelope.
pub fn call(method: &str, args: Value) -> Value {
    serde_json::json!({
        "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
        "methodCalls": [[method, args, "c0"]]
    })
}
