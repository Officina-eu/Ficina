//! Large-file transfer (Ficina Transfer): upload a file too big to attach and
//! get back a private, expiring **public** download link that rides the message
//! in place of an inline attachment.
//!
//! - `POST /share/upload?name=<file>` (authenticated) stores the bytes and mints
//!   a link, returning `{url, filename, size, expiresAt}`.
//! - `GET /share/{token}` (PUBLIC — the recipient may be anyone) serves the
//!   bytes as a download if the token is live, else a plain 404.
//!
//! Security: the token is 256-bit and stored hashed at rest; the download is
//! always `Content-Disposition: attachment` + `nosniff`, so a shared file is
//! never rendered inline. The link is a capability URL — holding it is the only
//! authorization, exactly the WeTransfer model.

use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header::CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use axum::{Json, response::Html};
use tokio::sync::Semaphore;
use ficina_store::StoreError;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::blob::serve_download;
use crate::error::Problem;
use crate::state::{AppState, authenticate};

/// The largest file accepted on the share path (buffered upload). Bigger than
/// the 50 MB attachment ceiling, but still bounded — true multi-GB streaming is
/// a tracked follow-up. Must be ≤ the blob store's configured max.
pub const SHARE_MAX_BYTES: usize = 100 * 1024 * 1024;

/// How long a share link lives before the sweeper reclaims it (14 days).
const SHARE_TTL_SECS: i64 = 14 * 24 * 60 * 60;

/// Cap on the stored filename length.
const MAX_FILENAME_LEN: usize = 255;

/// The public download route buffers up to `SHARE_MAX_BYTES` per request and is
/// unauthenticated, so a leaked/forwarded link could be used to force many
/// concurrent large reads. Cap how many downloads buffer at once (service-wide);
/// excess requests get a 503 to retry. Bounds the peak memory an attacker can
/// induce to `MAX_CONCURRENT_DOWNLOADS × SHARE_MAX_BYTES`.
const MAX_CONCURRENT_DOWNLOADS: usize = 6;
static DOWNLOAD_SLOTS: LazyLock<Semaphore> =
    LazyLock::new(|| Semaphore::new(MAX_CONCURRENT_DOWNLOADS));

#[derive(Deserialize)]
pub struct UploadQuery {
    /// The original filename (URL-encoded by the client; axum decodes it).
    name: Option<String>,
}

/// `POST /share/upload?name=<file>` — authenticated. Body is the raw file; the
/// `Content-Type` header is its media type. Returns the share link.
pub async fn upload(
    State(state): State<AppState>,
    Query(q): Query<UploadQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Problem> {
    let account = authenticate(&state, &headers).await?;
    if body.is_empty() {
        return Err(Problem::with(StatusCode::BAD_REQUEST, "empty file"));
    }
    if body.len() > SHARE_MAX_BYTES {
        return Err(Problem::with(StatusCode::PAYLOAD_TOO_LARGE, "file too large"));
    }
    let filename = sanitize_filename(q.name.as_deref());
    let content_type = sanitize_content_type(
        headers
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or(""),
    );
    let expires = now_epoch() + SHARE_TTL_SECS;

    let created = account
        .acc
        .create_share(body, &filename, &content_type, expires)
        .await
        .map_err(|e| match e {
            StoreError::TooLarge { .. } => {
                Problem::with(StatusCode::PAYLOAD_TOO_LARGE, "file too large")
            }
            StoreError::OverQuota => {
                Problem::with(StatusCode::INSUFFICIENT_STORAGE, "storage quota exceeded")
            }
            _ => Problem::server_error(),
        })?;

    let base = state.base_url.trim_end_matches('/');
    Ok(Json(json!({
        "url": format!("{base}/share/{}", created.token),
        "filename": filename,
        "size": created.size,
        "expiresAt": created.expires_at_epoch,
    })))
}

/// `GET /share/{token}` — PUBLIC. Serves the file as a download if the token is
/// live, otherwise a plain 404 page. No authentication: the unguessable token is
/// the capability.
pub async fn download(State(state): State<AppState>, Path(token): Path<String>) -> Response {
    // Bound concurrent large buffered reads on this unauthenticated path.
    let _permit = match DOWNLOAD_SLOTS.try_acquire() {
        Ok(permit) => permit,
        Err(_) => {
            return (StatusCode::SERVICE_UNAVAILABLE, "busy, try again shortly").into_response();
        }
    };
    let target = match state.store.resolve_share(&token).await {
        Ok(Some(t)) => t,
        _ => return not_found(),
    };
    match state.store.share_bytes(&target).await {
        Ok(bytes) => serve_download(bytes, &target.content_type, &target.filename),
        Err(_) => not_found(),
    }
}

/// A minimal 404 for an unknown or expired link (never reveals which).
fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Html("<!doctype html><meta charset=utf-8><title>Link expired</title><body style=\"font-family:system-ui;padding:3rem;color:#333\"><h1>This link has expired</h1><p>Ask the sender to share the file again.</p></body>"),
    )
        .into_response()
}

/// Sanitize a filename for storage + `Content-Disposition`: strip any path and
/// control/quote characters, cap the length, and never allow an empty name.
fn sanitize_filename(name: Option<&str>) -> String {
    let raw = name.unwrap_or("").trim();
    let base = raw.rsplit(['/', '\\']).next().unwrap_or(raw);
    let cleaned: String = base
        .chars()
        .filter(|c| !c.is_control() && *c != '"' && *c != '\\')
        .take(MAX_FILENAME_LEN)
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        "file".to_owned()
    } else {
        cleaned.to_owned()
    }
}

/// A safe media type: a plain `type/subtype`-ish token, else octet-stream. The
/// download is served as an attachment regardless, so this is belt-and-braces.
fn sanitize_content_type(raw: &str) -> String {
    let ct = raw.split(';').next().unwrap_or("").trim();
    let ok = !ct.is_empty()
        && ct.len() <= 128
        && ct.contains('/')
        && ct
            .bytes()
            .all(|b| b.is_ascii_graphic() && b != b'"' && b != b'\\');
    if ok {
        ct.to_ascii_lowercase()
    } else {
        "application/octet-stream".to_owned()
    }
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{sanitize_content_type, sanitize_filename};

    #[test]
    fn filename_strips_path_and_control_chars() {
        assert_eq!(sanitize_filename(Some("../../etc/passwd")), "passwd");
        assert_eq!(sanitize_filename(Some("a\r\nb\"c.pdf")), "abc.pdf");
        assert_eq!(sanitize_filename(Some("  ")), "file");
        assert_eq!(sanitize_filename(None), "file");
        assert_eq!(sanitize_filename(Some("report.xlsx")), "report.xlsx");
    }

    #[test]
    fn content_type_falls_back_when_junk() {
        assert_eq!(sanitize_content_type("image/png"), "image/png");
        assert_eq!(sanitize_content_type("application/pdf; charset=x"), "application/pdf");
        assert_eq!(sanitize_content_type("not a type"), "application/octet-stream");
        assert_eq!(sanitize_content_type(""), "application/octet-stream");
    }
}
