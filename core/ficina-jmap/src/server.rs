//! The axum router, the interim `/auth/token` endpoint, and `serve`.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use ficina_store::Store;
use serde_json::{Value, json};

use crate::error::Problem;
use crate::push::PushHub;
use crate::state::{AppState, Limits};
use crate::{api, blob, push, session};

/// Builds the JMAP router over the given state.
pub fn app(state: AppState) -> Router {
    let upload_limit = state.limits.max_size_upload as usize;
    let request_limit = state.limits.max_size_request;
    Router::new()
        .route("/.well-known/jmap", get(session::session))
        // The API route caps at maxSizeRequestObject; uploads get the
        // larger ceiling from the global layer below.
        .route(
            "/jmap/api",
            post(api::api).layer(DefaultBodyLimit::max(request_limit)),
        )
        .route("/auth/token", post(token))
        .route("/jmap/upload/{accountId}", post(blob::upload))
        .route(
            "/jmap/download/{accountId}/{blobId}/{name}",
            get(blob::download),
        )
        .route("/jmap/eventsource", get(push::event_source))
        .layer(DefaultBodyLimit::max(upload_limit))
        .with_state(state)
}

/// A convenience [`AppState`] with default limits and a fresh push hub.
pub fn app_state(store: Arc<Store>, base_url: impl Into<String>) -> AppState {
    AppState {
        store,
        push: PushHub::new(),
        limits: Limits::default(),
        base_url: base_url.into(),
    }
}

/// `POST /auth/token` — interim login (username/password) → bearer token.
async fn token(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, Problem> {
    let username = body
        .get("username")
        .and_then(Value::as_str)
        .ok_or_else(Problem::not_request)?;
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .ok_or_else(Problem::not_request)?;
    match state
        .store
        .issue_token(username, password)
        .await
        .map_err(|_| Problem::server_error())?
    {
        Some(issued) => Ok(Json(
            json!({ "token": issued.token, "accountId": issued.user.as_str() }),
        )),
        None => Err(Problem::unauthorized()),
    }
}

/// Binds `addr` and serves the JMAP API until shutdown.
///
/// # Errors
/// I/O errors binding or serving.
pub async fn serve(addr: SocketAddr, state: AppState) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "ficina-jmap listening");
    axum::serve(listener, app(state)).await
}
