//! Request-level problem details for the control plane. Internal error text
//! never reaches a client-visible `detail` (law #1: our logs and errors are
//! held to the promise we sell).

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// An HTTP request-level failure: a status code and a safe, client-visible
/// detail string (never internal error text).
#[derive(Debug)]
pub struct Problem {
    /// HTTP status.
    pub status: StatusCode,
    /// Optional human detail (safe to show a client).
    pub detail: Option<String>,
}

impl Problem {
    /// A problem with an explicit status and a safe detail.
    pub fn with(status: StatusCode, detail: impl Into<String>) -> Self {
        Self {
            status,
            detail: Some(detail.into()),
        }
    }

    /// Missing/invalid bearer token → 401.
    pub fn unauthorized() -> Self {
        Self::with(StatusCode::UNAUTHORIZED, "missing or invalid bearer token")
    }

    /// Authenticated but not a platform operator → 403.
    pub fn forbidden() -> Self {
        Self::with(StatusCode::FORBIDDEN, "platform operator only")
    }

    /// The request body was not valid JSON → 400.
    pub fn not_json() -> Self {
        Self::with(StatusCode::BAD_REQUEST, "request body must be JSON")
    }

    /// A required field was missing/invalid → 400.
    pub fn bad(detail: impl Into<String>) -> Self {
        Self::with(StatusCode::BAD_REQUEST, detail)
    }

    /// The addressed resource does not exist → 404.
    pub fn not_found() -> Self {
        Self::with(StatusCode::NOT_FOUND, "not found")
    }

    /// An internal failure → 500, with no leaked detail.
    pub fn server_error() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            detail: None,
        }
    }
}

impl IntoResponse for Problem {
    fn into_response(self) -> Response {
        let mut body = json!({ "status": self.status.as_u16() });
        if let Some(detail) = &self.detail {
            body["detail"] = json!(detail);
        }
        let mut resp = (self.status, Json(body)).into_response();
        if self.status == StatusCode::UNAUTHORIZED {
            resp.headers_mut().insert(
                axum::http::header::WWW_AUTHENTICATE,
                axum::http::HeaderValue::from_static("Bearer"),
            );
        }
        resp
    }
}
