//! The Session resource (RFC 8620 §2): capabilities, accounts, URLs, and
//! the honest, enforced limits.

use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use serde_json::{Map, Value, json};

use crate::error::Problem;
use crate::state::{AppState, authenticate};

const CAP_CORE: &str = "urn:ietf:params:jmap:core";
const CAP_MAIL: &str = "urn:ietf:params:jmap:mail";

/// `GET /.well-known/jmap` → the Session resource.
pub async fn session(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Problem> {
    let account = authenticate(&state, &headers).await?;
    let account_id = account.account_id().to_owned();
    let state_str = account
        .ts
        .state()
        .await
        .map_err(|_| Problem::server_error())?;
    let l = &state.limits;
    let base = &state.base_url;

    let mut accounts = Map::new();
    accounts.insert(
        account_id.clone(),
        json!({
            "name": account_id,
            "isPersonal": true,
            "isReadOnly": false,
            "accountCapabilities": { CAP_MAIL: {} }
        }),
    );
    let mut primary = Map::new();
    primary.insert(CAP_MAIL.to_owned(), json!(account_id));

    Ok(Json(json!({
        "capabilities": {
            CAP_CORE: {
                "maxSizeUpload": l.max_size_upload,
                "maxConcurrentUpload": l.max_concurrent_upload,
                "maxSizeRequestObject": l.max_size_request,
                "maxConcurrentRequests": 8,
                "maxCallsInRequest": l.max_calls_in_request,
                "maxObjectsInGet": l.max_objects_in_get,
                "maxObjectsInSet": l.max_objects_in_set,
                "collationAlgorithms": ["i;ascii-casemap", "i;unicode-casemap"]
            },
            CAP_MAIL: {
                "maxMailboxesPerEmail": Value::Null,
                "maxMailboxDepth": Value::Null,
                "maxSizeMailboxName": 490,
                "maxSizeAttachmentsPerEmail": l.max_size_upload,
                "emailQuerySortOptions": ["receivedAt"],
                "mayCreateTopLevelMailbox": true
            }
        },
        "accounts": accounts,
        "primaryAccounts": primary,
        "username": account_id,
        "apiUrl": format!("{base}/jmap/api"),
        "downloadUrl": format!("{base}/jmap/download/{{accountId}}/{{blobId}}/{{name}}"),
        "uploadUrl": format!("{base}/jmap/upload/{{accountId}}"),
        "eventSourceUrl": format!("{base}/jmap/eventsource?types={{types}}&closeafter={{closeafter}}&ping={{ping}}"),
        "state": state_str
    })))
}
