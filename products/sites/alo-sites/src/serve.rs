//! The public HTTP surface (ADR 0036, `docs/design/sites.md`): resolve the
//! Host header to one tenant's live site, serve its published snapshots, and
//! accept contact-form submissions (`POST /f/{form_id}`, the [`forms`]
//! module) — nothing else. The service holds no session — its whole tenant
//! scope is the [`host`] lookup's result (or, for a submission, the posted
//! form id's own resolution) — and it is deliberately terse on the wire:
//! misses are one uniform not-found, errors carry no internals.
//!
//! Response semantics:
//! - Pages and the stylesheet are immutable per publish, so 200s carry a
//!   strong `ETag` built from the publish id and honor `If-None-Match`
//!   with `304`. `Cache-Control: public, max-age=60` bounds client
//!   staleness; the service itself is never stale (the per-request resolver
//!   read is what flips content on republish).
//! - `/assets/img/<blob_id>` serves image bytes for exactly the blob ids
//!   the served publish references (tenant-scoped through the resolved
//!   site); immutable per id, so `max-age=3600` with the id as `ETag`.
//! - Unknown host → the generic not-found (identical for unknown and
//!   unpublished — no existence leak). Unknown path on a live site → the
//!   site's themed not-found. Both `404`, `no-cache`.
//! - Database trouble → `503` with a static line, `Retry-After: 10`.

mod cache;
pub mod config;
mod forms;
mod host;
mod rate;
mod rendered;

use std::sync::Arc;

use axum::Router;
use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};

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
    /// Per-client budget on the form-submit path (in-memory, transient).
    rate: rate::RateLimiter,
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
            rate: rate::RateLimiter::default(),
        })
    }
}

/// The service router: `/healthz`, the form-submit POST (with its own tight
/// body cap), and the catch-all site path.
pub fn app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route(
            "/f/{form_id}",
            post(forms::submit).layer(DefaultBodyLimit::max(forms::FORM_BODY_MAX_BYTES)),
        )
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

    // The image path of the public contract: bytes for exactly the blob ids
    // this publish references (`RenderedSite::serves_image`), read
    // tenant-scoped through the resolved site. Anything else — foreign,
    // unreferenced, or non-image — is the site's themed 404.
    if let Some(blob_id) = path.strip_prefix("/assets/img/") {
        if !site.serves_image(blob_id) {
            tracing::debug!(subdomain = %sub, "image not referenced by the served publish");
            return not_found(site.not_found.clone());
        }
        return serve_image(
            &state,
            &resolved,
            blob_id,
            req.headers(),
            site.not_found.clone(),
        )
        .await;
    }

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

/// Serves one referenced image blob. Bytes are immutable per blob id
/// (content-addressed underneath), so the ETag is the id and clients may
/// cache for an hour; the CSP defangs SVG opened as a top-level document
/// (no scripts can run on the site's origin).
async fn serve_image(
    state: &Arc<AppState>,
    resolved: &alo_store::PublishedSite,
    blob_id: &str,
    req_headers: &axum::http::HeaderMap,
    site_not_found: String,
) -> Response {
    let etag = format!("\"img:{blob_id}\"");
    if if_none_match_hits(req_headers.get(header::IF_NONE_MATCH), &etag) {
        // The bytes are immutable per id — a matching validator never needs
        // the row read at all.
        return image_response(StatusCode::NOT_MODIFIED, None, &etag, Vec::new());
    }
    match state.store.published_image(resolved, blob_id).await {
        Ok(Some(image)) => image_response(
            StatusCode::OK,
            Some(image.content_type),
            &etag,
            image.bytes.to_vec(),
        ),
        Ok(None) => {
            // Referenced by the publish but not servable (blob gone, or a
            // non-image content type): the same themed 404 as a missing page.
            tracing::warn!(site = %resolved.site, "referenced image blob is not servable");
            not_found(site_not_found)
        }
        Err(error) => {
            tracing::error!(site = %resolved.site, %error, "image read failed");
            unavailable()
        }
    }
}

/// A 200/304 for an image: revalidatable for an hour, `nosniff`, and a CSP
/// that keeps an SVG document inert on the site's origin.
fn image_response(
    status: StatusCode,
    content_type: Option<&'static str>,
    etag: &str,
    body: Vec<u8>,
) -> Response {
    let mut response = (status, body).into_response();
    let headers = response.headers_mut();
    if let Some(value) = content_type.and_then(|ct| HeaderValue::from_str(ct).ok()) {
        headers.insert(header::CONTENT_TYPE, value);
    }
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=3600"),
    );
    if let Ok(value) = HeaderValue::from_str(etag) {
        headers.insert(header::ETAG, value);
    }
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; style-src 'unsafe-inline'"),
    );
    response
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
