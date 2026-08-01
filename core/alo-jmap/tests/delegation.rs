//! Mailbox delegation over the wire (ADR 0017): a delegate can operate on the
//! owner's account only with a grant; without one it's the usual
//! accountNotFound (no oracle); the session advertises granted mailboxes; and a
//! read-only delegate cannot send.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use common::{api, harness};
use serde_json::{Value, json};
use tower::ServiceExt;

/// A one-call request targeting `account_id`.
fn call(account_id: &str, method: &str, mut args: Value) -> Value {
    args["accountId"] = json!(account_id);
    json!({
        "using": ["urn:ietf:params:jmap:core", "urn:ietf:params:jmap:mail"],
        "methodCalls": [[method, args, "c"]],
    })
}

fn err_type(body: &Value) -> Option<String> {
    body["methodResponses"][0]
        .as_array()
        .filter(|r| r[0] == json!("error"))
        .map(|r| r[1]["type"].as_str().unwrap_or("").to_owned())
}

#[tokio::test]
async fn delegate_access_requires_a_grant() {
    let h = harness("deleg").await;
    // A second user in the same tenant — the shared mailbox owner — with a
    // message in their inbox.
    let owner = h.ts.create_user("owner-deleg@example.test").await.unwrap();
    let owner_acc = h.store.for_account(h.tenant.clone(), owner.clone());
    owner_acc
        .deliver(b"From: a@x\r\nSubject: owner-secret\r\n\r\nbody\r\n")
        .await
        .unwrap();
    let owner_id = owner.to_string();

    // Without a grant, the delegate (the signed-in user) targeting the owner's
    // account gets accountNotFound — indistinguishable from a nonexistent id.
    let (status, body) = api(&h.app, &h.token, call(&owner_id, "Mailbox/get", json!({ "ids": null }))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(err_type(&body).as_deref(), Some("accountNotFound"));

    // Grant read access.
    h.ts.grant_delegate(&owner, &h.user, false).await.unwrap();

    // Now the delegate can read the owner's mailboxes...
    let (_s, body) = api(&h.app, &h.token, call(&owner_id, "Mailbox/get", json!({ "ids": null }))).await;
    let boxes = body["methodResponses"][0][1]["list"].as_array().unwrap();
    assert!(boxes.iter().any(|m| m["role"] == json!("inbox")), "owner's inbox visible");

    // ...and query the owner's messages.
    let (_s, body) = api(
        &h.app,
        &h.token,
        call(&owner_id, "Email/query", json!({ "filter": { "text": "owner-secret" } })),
    )
    .await;
    assert_eq!(body["methodResponses"][0][1]["ids"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn session_lists_granted_mailboxes() {
    let h = harness("deleg-sess").await;
    let owner = h.ts.create_user("owner-sess@example.test").await.unwrap();
    h.ts.grant_delegate(&owner, &h.user, true).await.unwrap();

    let req = Request::builder()
        .method("GET")
        .uri("/.well-known/jmap")
        .header("authorization", format!("Bearer {}", h.token))
        .body(Body::empty())
        .unwrap();
    let resp = h.app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    let session: Value = serde_json::from_slice(&bytes).unwrap();

    let shared = &session["accounts"][owner.to_string()];
    assert_eq!(shared["name"], json!("owner-sess@example.test"));
    assert_eq!(shared["isPersonal"], json!(false));
    assert_eq!(shared["alo:canSend"], json!(true));
}

#[tokio::test]
async fn read_only_delegate_cannot_send() {
    let h = harness("deleg-send").await;
    let owner = h.ts.create_user("owner-send@example.test").await.unwrap();
    h.ts.grant_delegate(&owner, &h.user, false).await.unwrap(); // read only
    let owner_id = owner.to_string();

    // An EmailSubmission on the owner's account is refused up front for a
    // read-only delegate (before any draft lookup).
    let (status, body) = api(
        &h.app,
        &h.token,
        call(&owner_id, "EmailSubmission/set", json!({ "create": { "s": { "emailId": "x" } } })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let created_err = &body["methodResponses"][0][1]["notCreated"]["s"]["type"];
    assert_eq!(created_err, &json!("forbiddenToSend"), "read-only delegate can't send: {body}");
}
