//! `POST /f/:form_id` — the contact-form submission endpoint, the one write
//! on the public surface (`docs/design/sites.md`, form flow). The wire
//! contract is terse and fixed: success and the honeypot's silent drop are
//! `200` with a plain "sent" page, an unresolvable form id is the generic
//! `404` (unknown, deleted, and draft-site forms are indistinguishable), an
//! unreadable or invalid body is `400`, an oversized body `413`, and a
//! rate-limited sender `429` with `Retry-After`. Responses are complete
//! little HTML documents because the no-script fallback navigates to them;
//! the scripted path only reads the status.
//!
//! Privacy: the client key for rate limiting is used transiently
//! ([`super::rate`]) and nothing about the visitor's connection is logged
//! or stored — a submission is exactly its three posted fields.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use axum::Form;
use axum::extract::rejection::FormRejection;
use axum::extract::{ConnectInfo, FromRequest, Path, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use alo_store::StoreError;

use crate::render::{EN, UiStrings};

use super::AppState;
use super::rendered::minimal_document;

/// The most an encoded submission body may carry. Sized from the store's
/// field caps at worst-case encoding (10k message characters, 4 UTF-8 bytes
/// each, tripled by percent-encoding) with headroom — a legitimate maximal
/// message always fits, a flood never does.
pub(super) const FORM_BODY_MAX_BYTES: usize = 256 * 1024;

/// The fixed v1 field contract of a rendered `contact_form` section.
/// Everything defaults to empty so a missing field fails in the store's
/// write gate with a field-level message, not as an opaque parse error;
/// `website` is the visually-hidden honeypot no human ever fills.
#[derive(Deserialize)]
struct SubmitBody {
    #[serde(default)]
    name: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    website: String,
}

/// Handles one submission POST end to end: rate limit, parse, honeypot,
/// then the store's conditional tenant-scoped write.
pub(super) async fn submit(
    State(state): State<Arc<AppState>>,
    Path(form_id): Path<String>,
    request: Request,
) -> Response {
    if let Err(wait) = state.rate.allow(&client_key(&request), Instant::now()) {
        return rate_limited(wait, &EN);
    }

    let body = match Form::<SubmitBody>::from_request(request, &()).await {
        Ok(Form(body)) => body,
        // The body never buffered (over the route's size limit): 413. Any
        // other rejection — wrong content type, broken urlencoding — is one
        // malformed-submission 400; the pages carry no parser internals.
        Err(FormRejection::BytesRejection(_)) => {
            return message_page(
                StatusCode::PAYLOAD_TOO_LARGE,
                &EN,
                EN.form_not_sent_title,
                EN.form_malformed_text,
            );
        }
        Err(_) => {
            return message_page(
                StatusCode::BAD_REQUEST,
                &EN,
                EN.form_not_sent_title,
                EN.form_malformed_text,
            );
        }
    };

    // Honeypot tripped: the field no human sees is filled, so this is bot
    // traffic. Answer exactly like success and write nothing — the bot
    // learns nothing and the owner's submissions stay clean.
    if !body.website.trim().is_empty() {
        return sent(&EN);
    }

    match state
        .store
        .add_public_form_submission(&form_id, &body.name, &body.email, &body.message)
        .await
    {
        Ok(Some(_)) => sent(&EN),
        Ok(None) => super::not_found(state.unknown_host.clone()),
        Err(StoreError::Validation(reason)) => {
            let text = format!("{reason}. {}", EN.form_back_hint);
            message_page(StatusCode::BAD_REQUEST, &EN, EN.form_not_sent_title, &text)
        }
        Err(error) => {
            tracing::error!(%error, "form submission write failed");
            super::unavailable()
        }
    }
}

/// The rate-limit key for this request: the last `X-Forwarded-For` entry —
/// the one appended by our own proxy, the only hop we trust — falling back
/// to the peer address when the service is reached directly. Used only
/// in-memory by the limiter; deliberately never logged.
fn client_key(request: &Request) -> String {
    if let Some(client) = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next_back())
        .map(str::trim)
        .filter(|client| !client.is_empty())
    {
        return client.to_owned();
    }
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map_or_else(|| "direct".to_owned(), |info| info.0.ip().to_string())
}

/// The one success answer — also the honeypot's silent drop, so the two are
/// indistinguishable on the wire.
fn sent(strings: &UiStrings) -> Response {
    message_page(
        StatusCode::OK,
        strings,
        strings.form_sent_title,
        strings.form_success,
    )
}

/// The 429, with the limiter's `Retry-After` hint in seconds.
fn rate_limited(wait_seconds: u64, strings: &UiStrings) -> Response {
    let mut response = message_page(
        StatusCode::TOO_MANY_REQUESTS,
        strings,
        strings.form_rate_limited_title,
        strings.form_rate_limited_text,
    );
    if let Ok(value) = HeaderValue::from_str(&wait_seconds.to_string()) {
        response.headers_mut().insert(header::RETRY_AFTER, value);
    }
    response
}

/// A form-result page: a minimal uncacheable HTML document, safe to land on
/// from a no-script form submit.
fn message_page(status: StatusCode, strings: &UiStrings, title: &str, text: &str) -> Response {
    (
        status,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            ),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
            (
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ),
        ],
        minimal_document(strings.lang, title, text),
    )
        .into_response()
}
