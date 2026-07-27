//! The JMAP API endpoint (RFC 8620 §3): the Request/Response envelope,
//! ordered method dispatch, result references, and the Mailbox/Email/
//! Thread methods mapped onto the tenant-scoped store.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::{Json, body::Bytes};
use ficina_store::{
    EmailFilter, EmailQuery, MailboxId, MessageId, Page, SortDirection, StoreError, ThreadId,
};
use serde_json::{Map, Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::error::{Problem, method_error, method_error_desc};
use crate::jtypes;
use crate::push::StateChangeMsg;
use crate::state::{Account, AppState, authenticate};

const CAP_CORE: &str = "urn:ietf:params:jmap:core";
const CAP_MAIL: &str = "urn:ietf:params:jmap:mail";
const CAP_SIEVE: &str = "urn:ietf:params:jmap:sieve";

/// `POST /jmap/api` — process a JMAP Request, return the Response.
pub async fn api(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, Problem> {
    if body.len() > state.limits.max_size_request {
        return Err(Problem::too_large());
    }
    let account = authenticate(&state, &headers).await?;

    let request: Value = serde_json::from_slice(&body).map_err(|_| Problem::not_json())?;
    let obj = request.as_object().ok_or_else(Problem::not_request)?;

    // `using` must list only capabilities we support.
    let using = obj
        .get("using")
        .and_then(Value::as_array)
        .ok_or_else(Problem::not_request)?;
    for cap in using {
        match cap.as_str() {
            Some(CAP_CORE) | Some(CAP_MAIL) | Some(CAP_SIEVE) => {}
            other => {
                return Err(Problem::unknown_capability().detail(other.unwrap_or("").to_owned()));
            }
        }
    }

    let method_calls = obj
        .get("methodCalls")
        .and_then(Value::as_array)
        .ok_or_else(Problem::not_request)?;
    if method_calls.len() > state.limits.max_calls_in_request {
        return Err(Problem::limit("too many method calls"));
    }

    let state_before = account.acc.state().await.unwrap_or_default();
    let mut responses: Vec<Value> = Vec::new();

    for call in method_calls {
        let (name, mut args, call_id) = match parse_invocation(call) {
            Some(triple) => triple,
            None => return Err(Problem::not_request()),
        };
        if let Err(reason) = resolve_references(&mut args, &responses) {
            responses.push(json!([
                "error",
                method_error_desc("invalidResultReference", &reason),
                call_id
            ]));
            continue;
        }
        match dispatch(&account, &state, &name, &args).await {
            Ok(result) => responses.push(json!([name, result, call_id])),
            Err(err) => responses.push(json!(["error", err, call_id])),
        }
    }

    // Push: if anything changed, notify this account's stream.
    let session_state = account
        .acc
        .state()
        .await
        .unwrap_or_else(|_| state_before.clone());
    if session_state != state_before {
        state.push.publish(
            account.tenant.as_str(),
            StateChangeMsg {
                account_id: account.account_id().to_owned(),
                types: vec![
                    ficina_store::changes::TYPE_MAILBOX,
                    ficina_store::changes::TYPE_EMAIL,
                    ficina_store::changes::TYPE_THREAD,
                ],
                state: session_state.clone(),
            },
        );
    }

    Ok(Json(
        json!({ "methodResponses": responses, "sessionState": session_state }),
    ))
}

/// Splits a method call `[name, args, callId]`.
fn parse_invocation(call: &Value) -> Option<(String, Value, Value)> {
    let arr = call.as_array()?;
    if arr.len() != 3 {
        return None;
    }
    Some((arr[0].as_str()?.to_owned(), arr[1].clone(), arr[2].clone()))
}

/// Resolves `#name` result references in an args object (RFC 8620 §3.7),
/// supporting plain JSON pointers and the `/*/prop` array-map form.
fn resolve_references(args: &mut Value, responses: &[Value]) -> Result<(), String> {
    let Some(obj) = args.as_object_mut() else {
        return Ok(());
    };
    let refs: Vec<String> = obj.keys().filter(|k| k.starts_with('#')).cloned().collect();
    for hashed in refs {
        let reference = obj.remove(&hashed).unwrap_or(Value::Null);
        let target = &hashed[1..];
        let result_of = reference.get("resultOf").and_then(Value::as_str);
        let name = reference.get("name").and_then(Value::as_str);
        let path = reference.get("path").and_then(Value::as_str);
        let (Some(result_of), Some(name), Some(path)) = (result_of, name, path) else {
            return Err(format!("malformed ResultReference for {target}"));
        };
        // Find the referenced prior response (matching callId + method).
        let source = responses.iter().rev().find_map(|r| {
            let a = r.as_array()?;
            if a.first()?.as_str()? == name && a.get(2)?.as_str()? == result_of {
                Some(a.get(1)?.clone())
            } else {
                None
            }
        });
        let source = source.ok_or_else(|| format!("no result for {result_of}/{name}"))?;
        let value = eval_path(&source, path).ok_or_else(|| format!("path {path} not found"))?;
        obj.insert(target.to_owned(), value);
    }
    Ok(())
}

/// Evaluates a JMAP reference path: a JSON pointer, optionally with one
/// `/*/` mapping an array element property.
fn eval_path(value: &Value, path: &str) -> Option<Value> {
    if let Some((prefix, suffix)) = path.split_once("/*/") {
        let array = value.pointer(prefix)?.as_array()?;
        let collected: Vec<Value> = array
            .iter()
            .filter_map(|el| el.pointer(&format!("/{suffix}")).cloned())
            .collect();
        return Some(Value::Array(collected));
    }
    value.pointer(path).cloned()
}

async fn dispatch(
    account: &Account,
    state: &AppState,
    name: &str,
    args: &Value,
) -> Result<Value, Value> {
    match name {
        "Core/echo" => Ok(args.clone()),
        "Mailbox/get" => mailbox_get(account, args).await,
        "Mailbox/set" => mailbox_set(account, args).await,
        "Mailbox/changes" => {
            changes(account, args, ficina_store::changes::TYPE_MAILBOX, state).await
        }
        "Email/get" => email_get(account, args, state).await,
        "Email/query" => email_query(account, args).await,
        "Email/set" => email_set(account, args).await,
        "Email/changes" => changes(account, args, ficina_store::changes::TYPE_EMAIL, state).await,
        "Thread/get" => thread_get(account, args).await,
        "SieveScript/get" => crate::sieve::get(account, args).await,
        "SieveScript/set" => crate::sieve::set(account, args).await,
        "SieveScript/validate" => crate::sieve::validate(account, args).await,
        _ => Err(method_error("unknownMethod")),
    }
}

// ---- account guard + helpers ------------------------------------------

fn check_account(args: &Value, account: &Account) -> Result<(), Value> {
    match args.get("accountId").and_then(Value::as_str) {
        Some(id) if id == account.account_id() => Ok(()),
        _ => Err(method_error("accountNotFound")),
    }
}

fn store_err(_e: StoreError) -> Value {
    method_error("serverFail")
}

/// Maps a store error on a per-object mutation to a JMAP SetError.
fn set_error(e: &StoreError) -> Value {
    match e {
        StoreError::NotFound => method_error("notFound"),
        StoreError::Conflict(msg) => method_error_desc("invalidProperties", msg),
        StoreError::TooLarge { .. } => method_error("tooLarge"),
        _ => method_error("serverFail"),
    }
}

// ---- Mailbox ----------------------------------------------------------

async fn mailbox_get(account: &Account, args: &Value) -> Result<Value, Value> {
    check_account(args, account)?;
    let state = account.acc.state().await.map_err(store_err)?;
    let ids = args.get("ids");

    let mut list = Vec::new();
    let mut not_found = Vec::new();
    if ids.is_none() || ids == Some(&Value::Null) {
        // All the account's mailboxes.
        let boxes = account
            .acc
            .mailboxes(Page::first(ficina_store::MAX_PAGE))
            .await
            .map_err(store_err)?;
        for m in &boxes {
            list.push(jtypes::mailbox_json(m));
        }
    } else {
        for id in ids.and_then(Value::as_array).into_iter().flatten() {
            let Some(id) = id.as_str() else { continue };
            let mid = MailboxId::new(id);
            // The account door is the scope: a foreign mailbox is NotFound.
            match account.acc.mailbox(&mid).await {
                Ok(m) => list.push(jtypes::mailbox_json(&m)),
                Err(StoreError::NotFound) => not_found.push(json!(id)),
                Err(e) => return Err(store_err(e)),
            }
        }
    }
    Ok(
        json!({ "accountId": account.account_id(), "state": state, "list": list, "notFound": not_found }),
    )
}

async fn mailbox_set(account: &Account, args: &Value) -> Result<Value, Value> {
    check_account(args, account)?;
    let old_state = account.acc.state().await.map_err(store_err)?;
    if let Some(expected) = args.get("ifInState").and_then(Value::as_str)
        && expected != old_state
    {
        return Err(method_error("stateMismatch"));
    }

    let (mut created, mut not_created) = (Map::new(), Map::new());
    let (mut updated, mut not_updated) = (Map::new(), Map::new());
    let (mut destroyed, mut not_destroyed) = (Vec::new(), Map::new());

    // create
    if let Some(creates) = args.get("create").and_then(Value::as_object) {
        for (cid, props) in creates {
            let name = props.get("name").and_then(Value::as_str).unwrap_or("");
            if name.is_empty() {
                not_created.insert(
                    cid.clone(),
                    method_error_desc("invalidProperties", "name required"),
                );
                continue;
            }
            let parent = props
                .get("parentId")
                .and_then(Value::as_str)
                .map(MailboxId::new);
            // A foreign parent is rejected by create_mailbox itself
            // (the account door scopes it) — no separate guard to forget.
            let role = props.get("role").and_then(Value::as_str);
            match account
                .acc
                .create_mailbox(parent.as_ref(), name, role)
                .await
            {
                Ok(id) => {
                    created.insert(cid.clone(), json!({ "id": id.as_str() }));
                }
                Err(e) => {
                    not_created.insert(cid.clone(), set_error(&e));
                }
            }
        }
    }

    // update (name / parentId)
    if let Some(updates) = args.get("update").and_then(Value::as_object) {
        for (id, patch) in updates {
            let mailbox = MailboxId::new(id.as_str());
            // Account-scoped existence check: a foreign mailbox is
            // NotFound, so an empty patch cannot report a spurious success.
            if let Err(e) = account.acc.mailbox(&mailbox).await {
                not_updated.insert(id.clone(), set_error(&e));
                continue;
            }
            let mut result: Result<(), StoreError> = Ok(());
            if let Some(name) = patch.get("name").and_then(Value::as_str) {
                result = account.acc.rename_mailbox(&mailbox, name).await;
            }
            if result.is_ok() && patch.get("parentId").is_some() {
                let parent = patch
                    .get("parentId")
                    .and_then(Value::as_str)
                    .map(MailboxId::new);
                result = account.acc.move_mailbox(&mailbox, parent.as_ref()).await;
            }
            match result {
                Ok(()) => {
                    updated.insert(id.clone(), Value::Null);
                }
                Err(e) => {
                    not_updated.insert(id.clone(), set_error(&e));
                }
            }
        }
    }

    // destroy
    if let Some(ids) = args.get("destroy").and_then(Value::as_array) {
        for id in ids {
            let Some(id) = id.as_str() else { continue };
            let mbox = MailboxId::new(id);
            // destroy_mailbox is account-scoped: a foreign mailbox is
            // NotFound → notDestroyed, no separate guard to forget.
            match account.acc.destroy_mailbox(&mbox).await {
                Ok(()) => destroyed.push(json!(id)),
                Err(e) => {
                    not_destroyed.insert(id.to_owned(), set_error(&e));
                }
            }
        }
    }

    let new_state = account.acc.state().await.map_err(store_err)?;
    Ok(json!({
        "accountId": account.account_id(), "oldState": old_state, "newState": new_state,
        "created": created, "updated": updated, "destroyed": destroyed,
        "notCreated": not_created, "notUpdated": not_updated, "notDestroyed": not_destroyed
    }))
}

// ---- Email ------------------------------------------------------------

async fn email_get(account: &Account, args: &Value, state: &AppState) -> Result<Value, Value> {
    check_account(args, account)?;
    let acct_state = account.acc.state().await.map_err(store_err)?;
    let want_body = args
        .get("fetchTextBodyValues")
        .and_then(Value::as_bool)
        .or_else(|| args.get("fetchAllBodyValues").and_then(Value::as_bool))
        .unwrap_or(false);
    let max_body = args
        .get("maxBodyValueBytes")
        .and_then(Value::as_u64)
        .map(|n| (n as usize).min(state.limits.max_body_value_bytes))
        .unwrap_or(state.limits.max_body_value_bytes);

    let mut list = Vec::new();
    let mut not_found = Vec::new();
    let ids: Vec<&str> = args
        .get("ids")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .take(state.limits.max_objects_in_get)
        .collect();

    for id in ids {
        let mid = MessageId::new(id);
        // The account door scopes every read: a foreign message is
        // NotFound from message() itself — no separate ownership guard.
        match account.acc.message(&mid).await {
            Ok(m) => {
                let mailbox_ids: Vec<String> = account
                    .acc
                    .mailboxes_of_message(&mid)
                    .await
                    .map_err(store_err)?
                    .into_iter()
                    .map(|b| b.to_string())
                    .collect();
                let keywords = account.acc.keywords(&mid).await.map_err(store_err)?;
                let body = if want_body {
                    let raw = account.acc.message_bytes(&mid).await.map_err(store_err)?;
                    Some(extract_text_body(&raw, max_body))
                } else {
                    None
                };
                let body_ref = body.as_ref().map(|(s, t)| (s.as_str(), *t));
                list.push(jtypes::email_json(
                    &m,
                    &mailbox_ids,
                    &keywords,
                    body_ref,
                    false,
                ));
            }
            Err(StoreError::NotFound) => not_found.push(json!(id)),
            Err(e) => return Err(store_err(e)),
        }
    }
    Ok(
        json!({ "accountId": account.account_id(), "state": acct_state, "list": list, "notFound": not_found }),
    )
}

async fn email_query(account: &Account, args: &Value) -> Result<Value, Value> {
    check_account(args, account)?;
    let query_state = account.acc.state().await.map_err(store_err)?;

    let filter = parse_email_filter(args.get("filter"));
    let sort = parse_sort(args.get("sort"));
    let position = args
        .get("position")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .max(0);
    let limit = args.get("limit").and_then(Value::as_i64).unwrap_or(50);
    let page = Page::new(limit, position);

    let query = EmailQuery { filter, sort, page };
    let results = account.acc.query_emails(&query).await.map_err(store_err)?;
    let ids: Vec<String> = results.iter().map(|m| m.id.to_string()).collect();

    Ok(json!({
        "accountId": account.account_id(),
        "queryState": query_state,
        "canCalculateChanges": false,
        "position": position,
        "ids": ids
    }))
}

async fn email_set(account: &Account, args: &Value) -> Result<Value, Value> {
    check_account(args, account)?;
    let old_state = account.acc.state().await.map_err(store_err)?;
    if let Some(expected) = args.get("ifInState").and_then(Value::as_str)
        && expected != old_state
    {
        return Err(method_error("stateMismatch"));
    }

    let (mut created, mut not_created) = (Map::new(), Map::new());
    let (mut updated, mut not_updated) = (Map::new(), Map::new());
    let (mut destroyed, mut not_destroyed) = (Vec::new(), Map::new());

    if let Some(creates) = args.get("create").and_then(Value::as_object) {
        for (cid, props) in creates {
            match email_create(account, props).await {
                Ok(created_obj) => {
                    created.insert(cid.clone(), created_obj);
                }
                Err(e) => {
                    not_created.insert(cid.clone(), e);
                }
            }
        }
    }

    if let Some(updates) = args.get("update").and_then(Value::as_object) {
        for (id, patch) in updates {
            // Account-scoped existence check: a foreign message is
            // NotFound, so an empty patch cannot report a spurious success.
            if let Err(e) = account.acc.message(&MessageId::new(id.as_str())).await {
                not_updated.insert(id.clone(), set_error(&e));
                continue;
            }
            match email_update(account, id, patch).await {
                Ok(()) => {
                    updated.insert(id.clone(), Value::Null);
                }
                Err(e) => {
                    not_updated.insert(id.clone(), e);
                }
            }
        }
    }

    if let Some(ids) = args.get("destroy").and_then(Value::as_array) {
        for id in ids {
            let Some(id) = id.as_str() else { continue };
            let mid = MessageId::new(id);
            // destroy_message is account-scoped: a foreign message is
            // NotFound → notDestroyed, no separate guard to forget.
            match account.acc.destroy_message(&mid).await {
                Ok(()) => destroyed.push(json!(id)),
                Err(e) => {
                    not_destroyed.insert(id.to_owned(), set_error(&e));
                }
            }
        }
    }

    let new_state = account.acc.state().await.map_err(store_err)?;
    Ok(json!({
        "accountId": account.account_id(), "oldState": old_state, "newState": new_state,
        "created": created, "updated": updated, "destroyed": destroyed,
        "notCreated": not_created, "notUpdated": not_updated, "notDestroyed": not_destroyed
    }))
}

/// Minimal draft create: builds a raw message from from/to/subject/
/// textBody and ingests it into the first target mailbox, applying the
/// requested keywords.
async fn email_create(account: &Account, props: &Value) -> Result<Value, Value> {
    let mailbox_ids = props.get("mailboxIds").and_then(Value::as_object);
    let Some(first_mailbox) = mailbox_ids.and_then(|m| m.keys().next()) else {
        return Err(method_error_desc(
            "invalidProperties",
            "mailboxIds required",
        ));
    };
    let mailbox = MailboxId::new(first_mailbox.as_str());
    // The account door scopes ingest: a foreign target mailbox is
    // NotFound from ingest itself — no separate ownership guard here.

    let subject = props.get("subject").and_then(Value::as_str).unwrap_or("");
    let from = props
        .get("from")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(|e| e.get("email"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let to = props
        .get("to")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(|e| e.get("email"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let text = props
        .get("bodyValues")
        .and_then(|bv| bv.as_object())
        .and_then(|m| m.values().next())
        .and_then(|v| v.get("value"))
        .and_then(Value::as_str)
        .unwrap_or("");
    // CR/LF-strip header values (no header injection).
    let clean = |s: &str| s.replace(['\r', '\n'], " ");
    let raw = format!(
        "From: {}\r\nTo: {}\r\nSubject: {}\r\n\r\n{}\r\n",
        clean(from),
        clean(to),
        clean(subject),
        text
    );

    let id = account
        .acc
        .ingest(&mailbox, raw.as_bytes())
        .await
        .map_err(|e| set_error(&e))?;
    // Drafts default to $draft; apply requested keywords (failures are
    // surfaced, not swallowed).
    account
        .acc
        .set_keyword(&id, "$draft", true)
        .await
        .map_err(|e| set_error(&e))?;
    if let Some(keywords) = props.get("keywords").and_then(Value::as_object) {
        for (kw, on) in keywords {
            account
                .acc
                .set_keyword(&id, kw, on.as_bool().unwrap_or(false))
                .await
                .map_err(|e| set_error(&e))?;
        }
    }
    // Return the server-set properties (real blobId/threadId/size).
    let m = account.acc.message(&id).await.map_err(|e| set_error(&e))?;
    Ok(json!({
        "id": m.id.as_str(),
        "blobId": m.blob_id.as_str(),
        "threadId": m.thread_id.as_str(),
        "size": m.size
    }))
}

/// Applies an Email/set update patch: full or patched `keywords` and
/// `mailboxIds`.
async fn email_update(account: &Account, id: &str, patch: &Value) -> Result<(), Value> {
    let mid = MessageId::new(id);
    let Some(obj) = patch.as_object() else {
        return Err(method_error("invalidPatch"));
    };

    // Full replacements first.
    if let Some(keywords) = obj.get("keywords").and_then(Value::as_object) {
        let current = account
            .acc
            .keywords(&mid)
            .await
            .map_err(|e| set_error(&e))?;
        for kw in &current {
            if !keywords.contains_key(kw) {
                account
                    .acc
                    .set_keyword(&mid, kw, false)
                    .await
                    .map_err(|e| set_error(&e))?;
            }
        }
        for (kw, on) in keywords {
            account
                .acc
                .set_keyword(&mid, kw, on.as_bool().unwrap_or(false))
                .await
                .map_err(|e| set_error(&e))?;
        }
    }
    if let Some(mailboxes) = obj.get("mailboxIds").and_then(Value::as_object) {
        let current: Vec<String> = account
            .acc
            .mailboxes_of_message(&mid)
            .await
            .map_err(|e| set_error(&e))?
            .into_iter()
            .map(|b| b.to_string())
            .collect();
        for existing in &current {
            if !mailboxes.contains_key(existing) {
                account
                    .acc
                    .remove_from_mailbox(&mid, &MailboxId::new(existing.as_str()))
                    .await
                    .map_err(|e| set_error(&e))?;
            }
        }
        for (mb, on) in mailboxes {
            if on.as_bool().unwrap_or(false) && !current.contains(mb) {
                account
                    .acc
                    .add_to_mailbox(&mid, &MailboxId::new(mb.as_str()))
                    .await
                    .map_err(|e| set_error(&e))?;
            }
        }
    }

    // Patch keys: `keywords/X` and `mailboxIds/X`.
    for (key, value) in obj {
        if let Some(kw) = key.strip_prefix("keywords/") {
            let on = value.as_bool().unwrap_or(!value.is_null());
            account
                .acc
                .set_keyword(&mid, kw, on)
                .await
                .map_err(|e| set_error(&e))?;
        } else if let Some(mb) = key.strip_prefix("mailboxIds/") {
            let mailbox = MailboxId::new(mb);
            if value.is_null() || value.as_bool() == Some(false) {
                account
                    .acc
                    .remove_from_mailbox(&mid, &mailbox)
                    .await
                    .map_err(|e| set_error(&e))?;
            } else {
                account
                    .acc
                    .add_to_mailbox(&mid, &mailbox)
                    .await
                    .map_err(|e| set_error(&e))?;
            }
        }
    }
    Ok(())
}

// ---- Thread -----------------------------------------------------------

async fn thread_get(account: &Account, args: &Value) -> Result<Value, Value> {
    check_account(args, account)?;
    let state = account.acc.state().await.map_err(store_err)?;
    let mut list = Vec::new();
    let mut not_found = Vec::new();
    for id in args
        .get("ids")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .take(500)
    {
        let tid = ThreadId::new(id);
        // The account door scopes thread_messages to this account's own
        // messages: a thread the account has no message in comes back
        // empty → notFound. No separate ownership guard.
        let members = account
            .acc
            .thread_messages(&tid, Page::first(ficina_store::MAX_PAGE))
            .await
            .map_err(store_err)?;
        if members.is_empty() {
            not_found.push(json!(id));
        } else {
            let email_ids: Vec<String> = members.iter().map(|m| m.to_string()).collect();
            list.push(jtypes::thread_json(id, &email_ids));
        }
    }
    Ok(
        json!({ "accountId": account.account_id(), "state": state, "list": list, "notFound": not_found }),
    )
}

// ---- /changes (shared) ------------------------------------------------

async fn changes(
    account: &Account,
    args: &Value,
    obj_type: &str,
    state: &AppState,
) -> Result<Value, Value> {
    check_account(args, account)?;
    let since = match args
        .get("sinceState")
        .and_then(Value::as_str)
        .and_then(parse_state)
    {
        Some(s) if s >= 0 => s,
        _ => return Err(method_error("cannotCalculateChanges")),
    };
    let current = account
        .acc
        .state()
        .await
        .map_err(store_err)?
        .parse::<i64>()
        .unwrap_or(0);
    if since > current {
        return Err(method_error("cannotCalculateChanges"));
    }
    let max = args
        .get("maxChanges")
        .and_then(Value::as_i64)
        .map(|n| n.clamp(1, state.limits.max_objects_in_get as i64))
        .unwrap_or(state.limits.max_objects_in_get as i64);

    let c = account
        .acc
        .changes(obj_type, since, max)
        .await
        .map_err(store_err)?;
    Ok(json!({
        "accountId": account.account_id(),
        "oldState": c.old_state.to_string(),
        "newState": c.new_state.to_string(),
        "hasMoreChanges": c.has_more,
        "created": c.created,
        "updated": c.updated,
        "destroyed": c.destroyed
    }))
}

// ---- parsing helpers --------------------------------------------------

fn parse_state(s: &str) -> Option<i64> {
    s.parse().ok()
}

fn parse_sort(sort: Option<&Value>) -> SortDirection {
    // Only `receivedAt` is a sort option; default newest-first.
    let ascending = sort
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(|c| c.get("isAscending"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if ascending {
        SortDirection::Asc
    } else {
        SortDirection::Desc
    }
}

fn parse_email_filter(filter: Option<&Value>) -> EmailFilter {
    let Some(f) = filter.and_then(Value::as_object) else {
        return EmailFilter::default();
    };
    let str_of = |k: &str| f.get(k).and_then(Value::as_str).map(str::to_owned);
    let date_of = |k: &str| {
        f.get(k)
            .and_then(Value::as_str)
            .and_then(|s| OffsetDateTime::parse(s, &Rfc3339).ok())
    };
    EmailFilter {
        in_mailbox: str_of("inMailbox").map(MailboxId::new),
        from: str_of("from"),
        to: str_of("to"),
        subject: str_of("subject"),
        text: str_of("text"),
        before: date_of("before"),
        after: date_of("after"),
        has_keyword: str_of("hasKeyword"),
        not_keyword: str_of("notKeyword"),
    }
}

/// Best-effort text body extraction: the bytes after the header/body
/// separator, lossily decoded and truncated to `max`. Full MIME
/// structure is additive later (design note out-of-scope).
fn extract_text_body(raw: &[u8], max: usize) -> (String, bool) {
    let body = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| &raw[i + 4..])
        .unwrap_or(&[]);
    let mut text = String::from_utf8_lossy(body).into_owned();
    let truncated = text.len() > max;
    if truncated {
        let mut end = max;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
    }
    (text, truncated)
}
