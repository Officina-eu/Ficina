//! Ficina multi-tenant control plane (ADR 0012).
//!
//! The operator surface — distinct from the tenant `/admin/*` surface on
//! `ficina-jmap` — for governing tenants across a shared deployment: list,
//! provision, suspend/resume, and delete tenants, and register/verify the
//! domains a tenant is allowed to assign addresses in. Every route is gated on
//! a platform operator (`users.is_platform_admin`); an operator token grants
//! governance only, never read access to any tenant's mail.
//!
//! The binary entry point is `main.rs`; all logic lives here (new-component
//! skill), so the router can be driven in-process by tests.

use std::net::SocketAddr;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post};

pub mod domains;
pub mod error;
pub mod state;
pub mod tenants;

pub use state::{ControlState, control_state};

/// The largest control-plane request body we accept — these are small JSON
/// documents, never uploads.
const MAX_BODY_BYTES: usize = 64 * 1024;

/// Builds the `/control/*` router over the given state.
pub fn app(state: ControlState) -> Router {
    Router::new()
        .route("/control/health", get(|| async { "ok" }))
        .route("/control/me", get(tenants::whoami))
        .route(
            "/control/tenants",
            get(tenants::list_tenants).post(tenants::create_tenant),
        )
        .route("/control/tenants/{id}/status", post(tenants::set_status))
        .route("/control/tenants/{id}", delete(tenants::delete_tenant))
        .route(
            "/control/domains",
            get(domains::list_domains).post(domains::create_domain),
        )
        .route("/control/domains/verify", post(domains::verify_domain))
        .route("/control/domains/delete", post(domains::delete_domain))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state)
}

/// Serves the control plane on `addr` until the process is stopped.
///
/// # Errors
/// Propagates a bind or serve failure.
pub async fn serve(addr: SocketAddr, state: ControlState) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app(state)).await
}
