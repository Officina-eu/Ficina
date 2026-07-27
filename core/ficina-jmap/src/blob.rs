//! JMAP blob upload/download (RFC 8620 §6). Blob ids are the store's —
//! no second id space. Upload enforces the size ceiling; download is
//! tenant-scoped and serves the stored Content-Type with no sniffing.

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Json, http::HeaderMap};
use ficina_store::{BlobId, StoreError};
use serde_json::{Value, json};

use crate::error::Problem;
use crate::state::{AppState, authenticate};

fn store_problem(e: StoreError) -> Problem {
    match e {
        StoreError::NotFound => Problem::not_found(),
        StoreError::TooLarge { .. } => Problem::too_large(),
        _ => Problem::server_error(),
    }
}

/// `POST /jmap/upload/{accountId}` — content-address the body into the
/// store's blob layer and return its blob id.
pub async fn upload(
    State(state): State<AppState>,
    Path(account_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Problem> {
    let account = authenticate(&state, &headers).await?;
    if account_id != account.account_id() {
        return Err(Problem::not_found());
    }
    if body.len() as u64 > state.limits.max_size_upload {
        return Err(Problem::too_large());
    }
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let size = body.len();
    let blob_id = account
        .ts
        .put_blob(body, content_type.as_deref())
        .await
        .map_err(store_problem)?;
    Ok(Json(json!({
        "accountId": account.account_id(),
        "blobId": blob_id.as_str(),
        "type": content_type.unwrap_or_else(|| "application/octet-stream".to_owned()),
        "size": size
    })))
}

/// `GET /jmap/download/{accountId}/{blobId}/{name}` — the blob's bytes,
/// tenant-scoped, served as an attachment with the stored type and
/// `nosniff`.
pub async fn download(
    State(state): State<AppState>,
    Path((account_id, blob_id, name)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let account = authenticate(&state, &headers).await?;
    if account_id != account.account_id() {
        return Err(Problem::not_found());
    }
    let id = BlobId::new(blob_id);
    // Account scope: the caller must own a message referencing this blob.
    account
        .ts
        .owns_blob(&account.user, &id)
        .await
        .map_err(store_problem)?;
    let meta = account.ts.blob(&id).await.map_err(store_problem)?;
    let bytes = account.ts.blob_bytes(&id).await.map_err(store_problem)?;

    let ctype = meta
        .content_type
        .unwrap_or_else(|| "application/octet-stream".to_owned());
    let mut resp = (StatusCode::OK, bytes).into_response();
    let h = resp.headers_mut();
    h.insert(
        CONTENT_TYPE,
        HeaderValue::from_str(&ctype)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    let filename = name.replace(['\r', '\n', '"', '\\'], "");
    h.insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
            .unwrap_or_else(|_| HeaderValue::from_static("attachment")),
    );
    Ok(resp)
}
