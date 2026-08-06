//! The "Ask alo" agent endpoints (ADR 0034) — the top-level agent that answers
//! or PROPOSES an action, plus the separate execute route that runs an APPROVED
//! action through the caller's tenant-scoped store.
//!
//! Two routes, one trust rule (ADR 0023/0034): `POST /ai/agent` **never acts** —
//! it returns an answer or a *proposed* action; `POST /ai/agent/execute` is the
//! **only** path that acts, and only for an action the user approved in the UI.
//! Both are authenticated and tenant-scoped: retrieval sees only what the caller
//! can see, and execution runs through `account.acc` (their tenant) — an agent
//! can never act outside the caller's own permissions.

use alo_ai::{AgentDecision, AiConfig, InferenceError, WorkspaceSource};
use alo_store::NewTask;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::{body::Bytes, Json};
use serde_json::{json, Value};

use crate::ai::MAX_ASK_BYTES;
use crate::error::Problem;
use crate::state::{authenticate, Account, AppState};

/// How many retrieved items ground one agent turn (mirrors `/ai/ask`).
const AGENT_SOURCES: i64 = 8;

/// `POST /ai/agent` — `{"q":"..."}` → `{"answer":str|null,
/// "action":{tool,args,say}|null, "sources":[...],
/// "reason":null|"unconfigured"|"unreachable"}`.
///
/// Runs access-scoped retrieval, then asks the model to answer **or** propose one
/// action. It never executes: a returned `action` is a proposal the UI must have
/// the user approve, which then calls `/ai/agent/execute`. Unlike `/ai/ask` it
/// still calls the model when retrieval is empty — the agent can act (e.g. create
/// a task) without any sources.
pub async fn agent(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Problem> {
    let account = authenticate(&state, &headers).await?;
    if body.len() > MAX_ASK_BYTES {
        return Err(Problem::with(StatusCode::PAYLOAD_TOO_LARGE, "request too large"));
    }
    let req: Value = serde_json::from_slice(&body).map_err(|_| Problem::not_json())?;
    let request = req
        .get("q")
        .or_else(|| req.get("request"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_owned();
    if request.is_empty() {
        return Err(Problem::with(StatusCode::BAD_REQUEST, "q required"));
    }

    // Access-scoped retrieval — the only thing the agent may ever see.
    let hits = account
        .acc
        .workspace_search_terms(&request, AGENT_SOURCES)
        .await
        .map_err(|_| Problem::server_error())?;
    let sources_json: Vec<Value> = hits
        .iter()
        .map(|h| json!({ "kind": h.kind, "id": h.id, "title": h.title, "space": h.space }))
        .collect();
    let ground: Vec<WorkspaceSource> = hits
        .iter()
        .enumerate()
        .map(|(i, h)| WorkspaceSource {
            index: i + 1,
            kind: h.kind.clone(),
            title: h.title.clone(),
            detail: String::new(),
        })
        .collect();

    let Some(row) = account
        .acc
        .default_ai_config()
        .await
        .map_err(|_| Problem::server_error())?
    else {
        return Ok(Json(json!({
            "answer": Value::Null, "action": Value::Null,
            "reason": "unconfigured", "sources": sources_json
        })));
    };
    let config = AiConfig {
        base_url: row.base_url,
        model: row.model,
        api_key: row.api_key,
        enabled: row.enabled,
    };
    match alo_ai::run_agent(&config, &request, &ground, &today_utc()).await {
        Ok(AgentDecision::Answer(answer)) => Ok(Json(json!({
            "answer": answer, "action": Value::Null,
            "reason": Value::Null, "sources": sources_json
        }))),
        Ok(AgentDecision::Action { action, say }) => Ok(Json(json!({
            "answer": Value::Null,
            "action": { "tool": action.tool, "args": action.args, "say": say },
            "reason": Value::Null, "sources": sources_json
        }))),
        Err(InferenceError::Disabled | InferenceError::NotConfigured) => Ok(Json(json!({
            "answer": Value::Null, "action": Value::Null,
            "reason": "unconfigured", "sources": sources_json
        }))),
        Err(_) => Ok(Json(json!({
            "answer": Value::Null, "action": Value::Null,
            "reason": "unreachable", "sources": sources_json
        }))),
    }
}

/// `POST /ai/agent/execute` — `{"tool":"create_task","args":{...}}` →
/// `{"ok":true,"result":{...}}`.
///
/// The **only** acting path. Validates the tool against the allowlist
/// ([`alo_ai::AGENT_TOOLS`]) and its args, then runs it through the caller's
/// tenant-scoped store. Called only after the user approved the proposed action.
pub async fn agent_execute(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Problem> {
    let account = authenticate(&state, &headers).await?;
    if body.len() > MAX_ASK_BYTES {
        return Err(Problem::with(StatusCode::PAYLOAD_TOO_LARGE, "request too large"));
    }
    let req: Value = serde_json::from_slice(&body).map_err(|_| Problem::not_json())?;
    let tool = req.get("tool").and_then(Value::as_str).unwrap_or("").trim();
    let args = req.get("args").cloned().unwrap_or(Value::Null);
    if !alo_ai::AGENT_TOOLS.contains(&tool) {
        return Err(Problem::with(StatusCode::BAD_REQUEST, "unknown tool"));
    }
    match tool {
        "create_task" => execute_create_task(&account, &args).await,
        // Unreachable given the allowlist check, but the match stays total.
        _ => Err(Problem::with(StatusCode::BAD_REQUEST, "unknown tool")),
    }
}

/// Create a task from approved args, on the caller's personal project. Reuses the
/// same tenant-scoped `create_task` the `/tasks` route uses — no new storage path.
async fn execute_create_task(account: &Account, args: &Value) -> Result<Json<Value>, Problem> {
    let title = args
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_owned();
    if title.is_empty() {
        return Err(Problem::with(StatusCode::UNPROCESSABLE_ENTITY, "title required"));
    }
    let description = args
        .get("notes")
        .and_then(Value::as_str)
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    let due_at = args.get("due").and_then(Value::as_str).and_then(parse_due);

    let project = account
        .acc
        .ensure_personal_project()
        .await
        .map_err(|_| Problem::server_error())?;
    let new = NewTask {
        title,
        description,
        status: None,
        assignee: None,
        due_at,
        priority: None,
        state: None, // active — the user approved it (not a "proposed" suggestion)
        source_kind: None,
        source_id: None,
    };
    let id = account
        .acc
        .create_task(&project, &new)
        .await
        .map_err(|_| Problem::server_error())?;
    Ok(Json(json!({
        "ok": true,
        "result": { "kind": "task", "id": id.as_str(), "title": new.title }
    })))
}

/// The current UTC date as `YYYY-MM-DD`, given to the agent so it can resolve
/// relative dates ("tomorrow") into absolute ones.
fn today_utc() -> String {
    let d = time::OffsetDateTime::now_utc().date();
    format!("{:04}-{:02}-{:02}", d.year(), u8::from(d.month()), d.day())
}

/// Parse a `YYYY-MM-DD` due date to midnight UTC, or `None` if malformed (a bad
/// date drops the due, never fails the task — the title is the essential part).
fn parse_due(s: &str) -> Option<time::OffsetDateTime> {
    let mut it = s.trim().split('-');
    let year: i32 = it.next()?.parse().ok()?;
    let month: u8 = it.next()?.parse().ok()?;
    let day: u8 = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    let month = time::Month::try_from(month).ok()?;
    let date = time::Date::from_calendar_date(year, month, day).ok()?;
    Some(date.midnight().assume_utc())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::parse_due;

    #[test]
    fn parses_iso_date_to_utc_midnight() {
        let d = parse_due("2026-08-07").unwrap();
        assert_eq!(d.year(), 2026);
        assert_eq!(d.month() as u8, 8);
        assert_eq!(d.day(), 7);
        assert_eq!(d.hour(), 0);
    }

    #[test]
    fn rejects_malformed_dates() {
        assert!(parse_due("not-a-date").is_none());
        assert!(parse_due("2026-13-01").is_none()); // month 13
        assert!(parse_due("2026-02-30").is_none()); // invalid day
        assert!(parse_due("2026-08").is_none()); // too few parts
        assert!(parse_due("2026-08-07-01").is_none()); // too many parts
    }
}
