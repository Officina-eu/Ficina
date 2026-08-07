//! The public HTTP surface (ADR 0036, `docs/design/sites.md`): resolve the
//! Host header to one tenant's live site, serve its published snapshots, and
//! nothing else. The service holds no session — its whole tenant scope is
//! the [`host`] lookup's result — and it is deliberately terse on the wire:
//! misses are one uniform not-found, errors carry no internals.
//!
//! Response semantics:
//! - Pages and the stylesheet are immutable per publish, so 200s carry a
//!   strong `ETag` built from the publish id and honor `If-None-Match`
//!   with `304`. `Cache-Control: public, max-age=60` bounds client
//!   staleness; the service itself is never stale (the per-request resolver
//!   read is what flips content on republish).
//! - Unknown host → the generic not-found (identical for unknown and
//!   unpublished — no existence leak). Unknown path on a live site → the
//!   site's themed not-found. Both `404`, `no-cache`.
//! - Database trouble → `503` with a static line, `Retry-After: 10`.

mod cache;
pub mod config;
mod host;
mod rendered;

use std::sync::Arc;

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use alo_store::SitePublicStore;

use crate::render::EN;
pub use config::{ConfigError, ServeConfig};
use rendered::RenderedSite;

/// Shared state of the public service.
pub struct AppState {
    store: SitePublicStore,
    sites_domain: String,
    cache: cache::SiteCache,
    /// The one body every host-level miss serves (built once).
    unknown_host: String,
}

impl AppState {
    /// Wires the service state: the public store door and the apex domain
    /// (already lowercase, from [`ServeConfig`]).
    #[must_use]
    pub fn new(store: SitePublicStore, sites_domain: String) -> Arc<Self> {
        Arc::new(Self {
            store,
            sites_domain,
            cache: cache::SiteCache::default(),
            unknown_host: rendered::unknown_host_not_found(&EN),
        })
    }
}

/// The service router: `/healthz` plus the catch-all site path.
pub fn app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .fallback(serve_site)
        .with_state(state)
}

/// Liveness: the process is up and routing. Deliberately does not touch the
/// database — a Postgres blip must not make the proxy mark every site dead.
async fn healthz() -> &'static str {
    "ok\n"
}

/// Serves one public request: Host → subdomain → current publish → bytes.
async fn serve_site(State(state): State<Arc<AppState>>, req: Request) -> Response {
    if req.method() != Method::GET && req.method() != Method::HEAD {
        return (
            StatusCode::METHOD_NOT_ALLOWED,
            [(header::ALLOW, HeaderValue::from_static("GET, HEAD"))],
            "method not allowed\n",
        )
            .into_response();
    }

    let Some(sub) = req
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| host::subdomain(value, &state.sites_domain))
    else {
        return not_found(state.unknown_host.clone());
    };

    let resolved = match state.store.resolve_published(&sub).await {
        Ok(Some(site)) => site,
        Ok(None) => return not_found(state.unknown_host.clone()),
        Err(error) => {
            tracing::error!(subdomain = %sub, %error, "resolver read failed");
            return unavailable();
        }
    };

    let site = match state.cache.get(&sub, &resolved.publish) {
        Some(site) => site,
        None => {
            let snapshots = match state.store.published_pages(&resolved).await {
                Ok(snapshots) => snapshots,
                Err(error) => {
                    tracing::error!(subdomain = %sub, %error, "snapshot read failed");
                    return unavailable();
                }
            };
            let built = Arc::new(RenderedSite::build(
                &sub,
                &state.sites_domain,
                &resolved,
                &snapshots,
            ));
            tracing::info!(
                subdomain = %sub,
                site = %resolved.site,
                publish = %resolved.publish,
                pages = snapshots.len(),
                "rendered publish into cache"
            );
            state.cache.put(&sub, Arc::clone(&built));
            built
        }
    };

    // `/about/` serves `/about` (the canonical URL in the document keeps
    // search engines on the slash-less form); everything else is exact.
    let raw = req.uri().path();
    let trimmed = raw.trim_end_matches('/');
    let path = if trimmed.is_empty() { "/" } else { trimmed };

    let (content_type, body) = if path == "/assets/site.css" {
        ("text/css; charset=utf-8", site.css.clone())
    } else if let Some(page) = site.page(path) {
        ("text/html; charset=utf-8", page.to_owned())
    } else {
        tracing::debug!(subdomain = %sub, "no page at requested path");
        return not_found(site.not_found.clone());
    };

    // Strong ETag: bytes are a pure function of (publish, path).
    let etag = format!("\"{}:{path}\"", site.publish.as_str());
    if if_none_match_hits(req.headers().get(header::IF_NONE_MATCH), &etag) {
        return cacheable(StatusCode::NOT_MODIFIED, content_type, &etag, String::new());
    }
    cacheable(StatusCode::OK, content_type, &etag, body)
}

/// Whether an `If-None-Match` value matches `etag` (list form and `*`).
fn if_none_match_hits(value: Option<&HeaderValue>, etag: &str) -> bool {
    value
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(',').any(|c| c.trim() == etag || c.trim() == "*"))
}

/// A 200/304 with the revalidation headers shared by pages and the stylesheet.
fn cacheable(status: StatusCode, content_type: &'static str, etag: &str, body: String) -> Response {
    let mut response = (status, body).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=60"),
    );
    if let Ok(value) = HeaderValue::from_str(etag) {
        headers.insert(header::ETAG, value);
    }
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

/// A 404 carrying the given document (generic or site-themed).
fn not_found(body: String) -> Response {
    (
        StatusCode::NOT_FOUND,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            ),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
            (
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ),
        ],
        body,
    )
        .into_response()
}

/// The terse 503 for database trouble — nothing internal on the wire.
fn unavailable() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/plain; charset=utf-8"),
            ),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
            (header::RETRY_AFTER, HeaderValue::from_static("10")),
        ],
        "temporarily unavailable\n",
    )
        .into_response()
}
