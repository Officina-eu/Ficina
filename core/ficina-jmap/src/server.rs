//! The axum router (JMAP methods + the mounted OIDC provider), the
//! non-public first-party `/auth/token` password grant, and `serve`.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use ficina_identity::Identity;
use ficina_store::Store;
use serde_json::{Value, json};

use crate::error::Problem;
use crate::push::PushHub;
use crate::state::{AppState, Limits};
use crate::{
    admin, ai, api, blob, contacts, docs, filters, push, schedule, security, session, settings,
    share, snooze,
};

/// Builds the JMAP router over the given state. The OpenID Connect /
/// OAuth 2.0 provider (`ficina-identity`) is mounted alongside so a Phase-1
/// deployment serves JMAP and the IdP from one HTTP service.
pub fn app(state: AppState) -> Router {
    let upload_limit = state.limits.max_size_upload as usize;
    let request_limit = state.limits.max_size_request;
    let identity_routes = ficina_identity::router(state.identity.clone());
    let jmap = Router::new()
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
        // AI inference (ADR 0011) — authenticated, tenant-scoped. Its own small
        // body limit: the draft cap, not the large blob-upload ceiling below.
        .route(
            "/ai/improve",
            post(ai::improve).layer(DefaultBodyLimit::max(ai::MAX_IMPROVE_BYTES)),
        )
        .route(
            "/ai/summarize",
            post(ai::summarize).layer(DefaultBodyLimit::max(ai::MAX_SUMMARIZE_BYTES)),
        )
        .route(
            "/ai/replies",
            post(ai::replies).layer(DefaultBodyLimit::max(ai::MAX_SUMMARIZE_BYTES)),
        )
        // Snooze: hide conversations until a chosen time (a background sweeper wakes them).
        .route("/snooze", post(snooze::snooze))
        // Send later: hold a draft until a chosen time (a background sweeper sends it).
        .route("/send-later", post(schedule::send_later))
        .route("/send-later/cancel", post(schedule::cancel_send))
        // Recent correspondents for compose recipient autocomplete.
        .route("/contacts", get(contacts::list))
        // Ficina Transfer: upload a large file (authenticated) for an expiring
        // link, and the PUBLIC download route the recipient's link points at.
        // The upload streams straight to storage, so its body limit is disabled
        // (there is no size cap); the handler never buffers the whole file.
        .route(
            "/share/upload",
            post(share::upload).layer(DefaultBodyLimit::disable()),
        )
        .route("/share/{token}", get(share::download))
        // Server-side mail filters (rules) + one-click Block sender.
        .route("/filters", get(filters::list).put(filters::save))
        .route("/filters/block", post(filters::block))
        // Ficina Docs (ADR 0015): tenant/owner-scoped technical-authoring documents.
        .route("/docs", get(docs::list).post(docs::create))
        .route(
            "/docs/{id}",
            get(docs::get)
                .put(docs::save)
                .delete(docs::delete)
                .layer(DefaultBodyLimit::max(docs::MAX_DOC_BYTES)),
        )
        // Admin console (tenant-admin only): AI provider management.
        .route(
            "/admin/ai/providers",
            get(admin::list_providers).post(admin::upsert_provider),
        )
        .route("/admin/ai/providers/default", post(admin::set_default))
        .route(
            "/admin/ai/providers/{id}",
            axum::routing::delete(admin::delete_provider),
        )
        .route("/admin/ai/test", post(admin::test_connection))
        // Admin console: users & mailboxes.
        .route(
            "/admin/users",
            get(admin::list_users).post(admin::create_user),
        )
        .route("/admin/users/password", post(admin::reset_password))
        .route("/admin/users/admin", post(admin::set_user_admin))
        .route("/admin/users/alias", post(admin::add_alias))
        .route("/admin/users/alias/remove", post(admin::remove_alias))
        .route(
            "/admin/users/{id}",
            axum::routing::delete(admin::delete_user),
        )
        // Admin console: groups & lists.
        .route(
            "/admin/groups",
            get(admin::list_groups).post(admin::create_group),
        )
        .route("/admin/groups/address", post(admin::set_group_address))
        .route("/admin/groups/members", post(admin::add_group_member))
        .route(
            "/admin/groups/members/remove",
            post(admin::remove_group_member),
        )
        .route(
            "/admin/groups/{id}",
            axum::routing::delete(admin::delete_group),
        )
        // Admin console: this tenant's domains (register + DNS-verify).
        .route(
            "/admin/domains",
            get(admin::list_domains).post(admin::create_domain),
        )
        .route("/admin/domains/verify", post(admin::verify_domain))
        .route("/admin/domains/delete", post(admin::delete_domain))
        .route("/admin/domains/dkim/rotate", post(admin::rotate_dkim))
        // Admin console: security & trust (live deliverability checks).
        .route("/admin/security/checks", get(security::checks))
        // Admin console: audit log (this tenant's administrative actions).
        .route("/admin/audit", get(admin::list_audit))
        // Mail settings: the user's signature + the tenant footer.
        .route("/settings/mail", get(settings::mail_settings))
        .route("/settings/signature", post(settings::set_signature))
        .route("/settings/out-of-office", post(settings::set_out_of_office))
        .route("/admin/org-footer", post(settings::set_org_footer))
        .layer(DefaultBodyLimit::max(upload_limit))
        .with_state(state);
    jmap.merge(identity_routes)
}

/// A convenience [`AppState`] with default limits and a fresh push hub.
pub fn app_state(store: Arc<Store>, identity: Identity, base_url: impl Into<String>) -> AppState {
    AppState {
        store,
        identity,
        push: PushHub::new(),
        limits: Limits::default(),
        base_url: base_url.into(),
        submission_addr: std::env::var("FICINA_JMAP_SUBMISSION_ADDR").ok(),
    }
}

/// `POST /auth/token` — the **non-public** first-party password grant for
/// programmatic clients (e.g. the raw JMAP exit-gate client): username +
/// password (+ optional `otp`) → an opaque access token, issued through
/// `ficina-identity` with the same constant-time path and 2FA enforcement
/// as the OAuth flow. Public/browser clients use `/oauth/authorize`
/// instead (ADR 0008).
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
    let otp = body.get("otp").and_then(Value::as_str);
    match state
        .identity
        .password_login(username, password, otp)
        .await
        .map_err(|_| Problem::server_error())?
    {
        Some((token, principal)) => Ok(Json(
            json!({ "token": token.reveal(), "accountId": principal.user.as_str() }),
        )),
        None => Err(Problem::unauthorized()),
    }
}

/// Binds `addr` and serves the JMAP API (with the OIDC provider) until
/// shutdown. Provisions an ID-token signing key first (idempotent), so the
/// mounted `/oauth/jwks` and token-signing paths work without an
/// out-of-band CLI step — failing fast with a clear message if it cannot.
///
/// # Errors
/// I/O errors binding or serving; a startup error if the signing key cannot
/// be provisioned.
pub async fn serve(addr: SocketAddr, state: AppState) -> std::io::Result<()> {
    state.identity.ensure_signing_key().await.map_err(|error| {
        std::io::Error::other(format!("could not provision OIDC signing key: {error}"))
    })?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "ficina-jmap listening");
    axum::serve(listener, app(state)).await
}
