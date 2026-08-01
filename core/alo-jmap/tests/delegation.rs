//! Mailbox delegation over the wire (ADR 0017): a delegate operates on the
//! owner's account only with a grant (else accountNotFound, no oracle); a
//! read-only delegate can read but not mutate; a manage delegate can mutate but
//! not send without a send grant; the session reflects the level; and a user
//! can share their own mailbox self-service.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use common::{api, harness};
use serde_json::{Value, json};
use tower::ServiceExt;

fn call(account_id: &str, method: &str, mut args: Value) -> Value {
    args["accountId"] = json!(account_id);
    json!({
        "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
        "methodCalls": [[method, args, "c"]],
    })
}

fn resp_name(body: &Value) -> String {
    body["methodResponses"][0][0].as_str().unwrap_or("").to_owned()
}
fn err_type(body: &Value) -> String {
    body["methodResponses"][0][1]["type"].as_str().unwrap_or("").to_owned()
}

#[tokio::test]
async fn read_only_delegate_can_read_not_write() {
    let h = harness("deleg-ro").await;
    let owner = h.ts.create_user("owner-ro@example.test").await.unwrap();
    let owner_acc = h.store.for_account(h.tenant.clone(), owner.clone());
    let mid = owner_acc
        .deliver(b"From: a@x\r\nSubject: owner-secret\r\n\r\nbody\r\n")
        .await
        .unwrap();
    let owner_id = owner.to_string();

    // No grant → accountNotFound.
    let (_s, body) = api(&h.app, &h.token, call(&owner_id, "Mailbox/get", json!({ "ids": null }))).await;
    assert_eq!(err_type(&body), "accountNotFound");

    // Read-only grant.
    h.ts.grant_delegate(&owner, &h.user, false, "none").await.unwrap();

    // Can read the owner's mail...
    let (_s, body) = api(&h.app, &h.token, call(&owner_id, "Mailbox/get", json!({ "ids": null }))).await;
    assert_eq!(resp_name(&body), "Mailbox/get");

    // ...but any /set is refused as read-only.
    let update = json!({ mid.to_string(): { "keywords/$flagged": true } });
    let (_s, body) = api(&h.app, &h.token, call(&owner_id, "Email/set", json!({ "update": update }))).await;
    assert_eq!(err_type(&body), "accountReadOnly", "read-only delegate can't mutate: {body}");
}

#[tokio::test]
async fn manage_delegate_writes_but_cannot_send_without_grant() {
    let h = harness("deleg-manage").await;
    let owner = h.ts.create_user("owner-mng@example.test").await.unwrap();
    let owner_acc = h.store.for_account(h.tenant.clone(), owner.clone());
    let mid = owner_acc
        .deliver(b"From: a@x\r\nSubject: s\r\n\r\nb\r\n")
        .await
        .unwrap();
    let owner_id = owner.to_string();

    // Manage access, but no send.
    h.ts.grant_delegate(&owner, &h.user, true, "none").await.unwrap();

    // Can flag a message in the owner's mailbox.
    let update = json!({ mid.to_string(): { "keywords/$flagged": true } });
    let (_s, body) = api(&h.app, &h.token, call(&owner_id, "Email/set", json!({ "update": update }))).await;
    assert!(
        body["methodResponses"][0][1]["updated"].get(mid.to_string()).is_some(),
        "manage delegate can write: {body}",
    );

    // But cannot send (no send grant) — refused up front.
    let (_s, body) = api(
        &h.app,
        &h.token,
        call(&owner_id, "EmailSubmission/set", json!({ "create": { "s": { "emailId": "x" } } })),
    )
    .await;
    assert_eq!(
        body["methodResponses"][0][1]["notCreated"]["s"]["type"],
        json!("forbiddenToSend"),
    );
}

#[tokio::test]
async fn session_reflects_access_level() {
    let h = harness("deleg-sess").await;
    let send_owner = h.ts.create_user("send-owner@example.test").await.unwrap();
    let ro_owner = h.ts.create_user("ro-owner@example.test").await.unwrap();
    h.ts.grant_delegate(&send_owner, &h.user, true, "on_behalf").await.unwrap();
    h.ts.grant_delegate(&ro_owner, &h.user, false, "none").await.unwrap();

    let req = Request::builder()
        .method("GET")
        .uri("/.well-known/jmap")
        .header("authorization", format!("Bearer {}", h.token))
        .body(Body::empty())
        .unwrap();
    let resp = h.app.clone().oneshot(req).await.unwrap();
    let bytes = to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    let session: Value = serde_json::from_slice(&bytes).unwrap();

    let send = &session["accounts"][send_owner.to_string()];
    assert_eq!(send["isPersonal"], json!(false));
    assert_eq!(send["isReadOnly"], json!(false));
    assert_eq!(send["alo:canSend"], json!(true));

    let ro = &session["accounts"][ro_owner.to_string()];
    assert_eq!(ro["isReadOnly"], json!(true));
    assert_eq!(ro["alo:canSend"], json!(false));
}

#[tokio::test]
async fn self_service_share_and_revoke() {
    let h = harness("deleg-self").await;
    // A colleague in the same tenant.
    let colleague_email = format!("colleague-{}@example.test", h.tenant);
    h.ts.create_user(&colleague_email).await.unwrap();

    // The signed-in user shares THEIR OWN mailbox with the colleague (no admin).
    let (status, _b) = post(&h.app, &h.token, "/jmap/delegates",
        json!({ "email": colleague_email, "canWrite": true, "sendMode": "as" })).await;
    assert_eq!(status, StatusCode::OK, "self-service grant");

    // It shows up in the owner's delegate list.
    let (status, body) = get(&h.app, &h.token, "/jmap/delegates").await;
    assert_eq!(status, StatusCode::OK);
    let ds = body["delegates"].as_array().unwrap();
    assert_eq!(ds.len(), 1);
    assert_eq!(ds[0]["email"], json!(colleague_email));
    assert_eq!(ds[0]["sendMode"], json!("as"));
    let colleague_id = ds[0]["id"].as_str().unwrap().to_owned();

    // Sharing with a stranger (not in the tenant) is a not-found.
    let (status, _b) = post(&h.app, &h.token, "/jmap/delegates",
        json!({ "email": "nobody@example.test" })).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Revoke.
    let (status, _b) = post(&h.app, &h.token, "/jmap/delegates/remove",
        json!({ "delegateId": colleague_id })).await;
    assert_eq!(status, StatusCode::OK);
    let (_s, body) = get(&h.app, &h.token, "/jmap/delegates").await;
    assert!(body["delegates"].as_array().unwrap().is_empty());
}

async fn post(app: &axum::Router, token: &str, uri: &str, body: Value) -> (StatusCode, Value) {
    send_req(app, token, "POST", uri, Some(body)).await
}
async fn get(app: &axum::Router, token: &str, uri: &str) -> (StatusCode, Value) {
    send_req(app, token, "GET", uri, None).await
}
async fn send_req(
    app: &axum::Router,
    token: &str,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let b = body.map(|v| Body::from(v.to_string())).unwrap_or(Body::empty());
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(b)
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}
