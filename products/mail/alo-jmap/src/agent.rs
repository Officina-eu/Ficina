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
use alo_store::{CalendarEvent, EventId, MailboxId, MessageId, NewTask, Page, MAX_PAGE};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::{body::Bytes, Json};
use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;

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
    // (kind, id, title) per retrieved item, so a proposed email action referring
    // to a source by its number can be resolved to the concrete message id here —
    // execute never re-searches, and the model never sees raw ids.
    let sources: Vec<(String, String, String)> = hits
        .iter()
        .map(|h| (h.kind.clone(), h.id.clone(), h.title.clone()))
        .collect();
    match alo_ai::run_agent(&config, &request, &ground, &today_utc()).await {
        Ok(AgentDecision::Answer(answer)) => Ok(Json(json!({
            "answer": answer, "action": Value::Null,
            "reason": Value::Null, "sources": sources_json
        }))),
        Ok(AgentDecision::Action { mut action, say }) => {
            resolve_email_source(&mut action.args, &sources);
            Ok(Json(json!({
                "answer": Value::Null,
                "action": { "tool": action.tool, "args": action.args, "say": say },
                "reason": Value::Null, "sources": sources_json
            })))
        }
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
        "create_event" => execute_create_event(&account, &args).await,
        "mark_read" => execute_set_keyword(&account, &args, "$seen", "read").await,
        "flag_email" => execute_set_keyword(&account, &args, "$flagged", "flagged").await,
        "archive_email" => execute_archive(&account, &args).await,
        // Unreachable given the allowlist check, but the match stays total.
        _ => Err(Problem::with(StatusCode::BAD_REQUEST, "unknown tool")),
    }
}

/// Replace `{"source": n}` in an action's args with the concrete email it refers
/// to (`message_id` + `subject`), from the retrieval results. Only resolves when
/// the referenced source is an email; leaves non-email or source-less args
/// untouched. Pure so the mapping is unit-tested.
fn resolve_email_source(args: &mut Value, sources: &[(String, String, String)]) {
    let Some(n) = args.get("source").and_then(Value::as_u64) else {
        return;
    };
    let Some((kind, id, title)) = (n as usize).checked_sub(1).and_then(|i| sources.get(i)) else {
        return;
    };
    if kind != "message" {
        return;
    }
    if let Some(obj) = args.as_object_mut() {
        obj.remove("source");
        obj.insert("message_id".to_owned(), json!(id));
        obj.insert("subject".to_owned(), json!(title));
    }
}

/// Read the resolved `message_id` from an email action's args.
fn message_id_arg(args: &Value) -> Result<MessageId, Problem> {
    let id = args
        .get("message_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if id.is_empty() {
        return Err(Problem::with(StatusCode::UNPROCESSABLE_ENTITY, "message required"));
    }
    Ok(MessageId::new(id.to_owned()))
}

/// Set or clear a keyword ($seen for read, $flagged for flag) on an email.
async fn execute_set_keyword(
    account: &Account,
    args: &Value,
    keyword: &str,
    flag_field: &str,
) -> Result<Json<Value>, Problem> {
    let msg = message_id_arg(args)?;
    // Default the boolean to true ("mark read" / "flag" without an explicit value).
    let on = args.get(flag_field).and_then(Value::as_bool).unwrap_or(true);
    account
        .acc
        .set_keyword(&msg, keyword, on)
        .await
        .map_err(|_| Problem::with(StatusCode::UNPROCESSABLE_ENTITY, "could not update the email"))?;
    Ok(Json(json!({ "ok": true, "result": { "kind": "email", "id": msg.as_str() } })))
}

/// Archive an email: add it to Archive and take it out of the Inbox.
async fn execute_archive(account: &Account, args: &Value) -> Result<Json<Value>, Problem> {
    let msg = message_id_arg(args)?;
    let boxes = account
        .acc
        .mailboxes(Page::first(MAX_PAGE))
        .await
        .map_err(|_| Problem::server_error())?;
    let by_role = |role: &str| {
        boxes
            .iter()
            .find(|m| m.role.as_deref() == Some(role))
            .map(|m| MailboxId::new(m.id.as_str()))
    };
    // Get-or-create the Archive mailbox, the same on-demand idiom every other
    // standard role uses (Inbox, Drafts, Snoozed, Scheduled) — a first archive on
    // an account that never had the folder should succeed, not fail.
    let archive = match by_role("archive") {
        Some(id) => id,
        None => account
            .acc
            .create_mailbox(None, "Archive", Some("archive"))
            .await
            .map_err(|_| Problem::with(StatusCode::UNPROCESSABLE_ENTITY, "could not archive the email"))?,
    };
    account
        .acc
        .add_to_mailbox(&msg, &archive)
        .await
        .map_err(|_| Problem::with(StatusCode::UNPROCESSABLE_ENTITY, "could not archive the email"))?;
    // Take it out of the Inbox; if it was not there, the archive still stands.
    if let Some(inbox) = by_role("inbox") {
        match account.acc.remove_from_mailbox(&msg, &inbox).await {
            Ok(()) | Err(_) => {}
        }
    }
    Ok(Json(json!({ "ok": true, "result": { "kind": "email", "id": msg.as_str() } })))
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

/// Schedule a calendar event from approved args, on the caller's personal
/// calendar. Reuses the same tenant-scoped `create_event` the `/calendar/events`
/// route uses (which checks edit permission on the calendar) — no new path.
async fn execute_create_event(account: &Account, args: &Value) -> Result<Json<Value>, Problem> {
    let title = args
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_owned();
    if title.is_empty() {
        return Err(Problem::with(StatusCode::UNPROCESSABLE_ENTITY, "title required"));
    }
    let starts_at = args
        .get("start")
        .and_then(Value::as_str)
        .and_then(parse_rfc3339)
        .ok_or_else(|| Problem::with(StatusCode::UNPROCESSABLE_ENTITY, "a valid RFC 3339 start is required"))?;
    // End defaults to one hour after start; a given end before start is ignored.
    let ends_at = args
        .get("end")
        .and_then(Value::as_str)
        .and_then(parse_rfc3339)
        .filter(|e| *e >= starts_at)
        .unwrap_or_else(|| starts_at + time::Duration::hours(1));
    let clean = |k: &str| {
        args.get(k)
            .and_then(Value::as_str)
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
    };

    let calendar_id = account
        .acc
        .ensure_personal_calendar()
        .await
        .map_err(|_| Problem::server_error())?;
    let event = CalendarEvent {
        id: EventId::generate(),
        calendar_id,
        summary: title.clone(),
        description: clean("notes"),
        location: clean("location"),
        starts_at,
        ends_at,
        all_day: false,
        recurrence: None,
        attendees: Vec::new(),
        exdates: Vec::new(),
        recurrence_id: None,
        reminder_minutes: None,
        attendee_status: Vec::new(),
    };
    let id = account
        .acc
        .create_event(&event)
        .await
        .map_err(|_| Problem::server_error())?;
    Ok(Json(json!({
        "ok": true,
        "result": { "kind": "event", "id": id.as_str(), "title": title }
    })))
}

/// Parse an RFC 3339 datetime to UTC, or `None` if malformed.
fn parse_rfc3339(s: &str) -> Option<time::OffsetDateTime> {
    time::OffsetDateTime::parse(s.trim(), &Rfc3339)
        .ok()
        .map(|t| t.to_offset(time::UtcOffset::UTC))
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
    use super::{parse_due, parse_rfc3339, resolve_email_source};

    #[test]
    fn resolves_an_email_source_number_to_its_message_id() {
        let sources = vec![
            ("file".to_owned(), "f1".to_owned(), "Report".to_owned()),
            ("message".to_owned(), "m2".to_owned(), "Re: Acme".to_owned()),
        ];
        // An email source number becomes the concrete message id + subject.
        let mut args = serde_json::json!({ "source": 2, "read": true });
        resolve_email_source(&mut args, &sources);
        assert_eq!(args["message_id"], "m2");
        assert_eq!(args["subject"], "Re: Acme");
        assert!(args.get("source").is_none());
        assert_eq!(args["read"], true);
        // A non-email source (a file) is left as-is — execute then rejects it.
        let mut file = serde_json::json!({ "source": 1 });
        resolve_email_source(&mut file, &sources);
        assert!(file.get("message_id").is_none());
        // No "source" at all (e.g. create_task) — untouched.
        let mut task = serde_json::json!({ "title": "x" });
        resolve_email_source(&mut task, &sources);
        assert_eq!(task, serde_json::json!({ "title": "x" }));
    }

    #[test]
    fn parses_rfc3339_to_utc() {
        let t = parse_rfc3339("2026-08-07T14:30:00Z").unwrap();
        assert_eq!(t.hour(), 14);
        assert_eq!(t.minute(), 30);
        // An offset time normalises to UTC.
        let z = parse_rfc3339("2026-08-07T16:30:00+02:00").unwrap();
        assert_eq!(z.hour(), 14);
        assert!(parse_rfc3339("2026-08-07").is_none()); // date only, not a datetime
        assert!(parse_rfc3339("not-a-time").is_none());
    }

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
