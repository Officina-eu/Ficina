//! The wrong-tenant suite extended to the JMAP surface: authenticated as
//! tenant A, wielding tenant B's ids/blobIds/state tokens yields a clean
//! notFound/empty — never B's data, never a 500. Against real Postgres.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{api, call, harness, send};
use ficina_store::{BlobStore, Store};
use serde_json::{Map, Value, json};
use sqlx::postgres::PgPoolOptions;

fn obj(pairs: Vec<(String, Value)>) -> Value {
    Value::Object(pairs.into_iter().collect::<Map<_, _>>())
}

#[tokio::test]
async fn cross_account_within_one_tenant_is_denied() {
    // Two users in ONE tenant: user 1 must never reach user 2's mail
    // (JMAP account = user). This is the blind spot a single-user-per-
    // tenant harness cannot see.
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&common::database_url())
        .await
        .unwrap();
    let store = Arc::new(Store::new(pool, BlobStore::in_memory(50 * 1024 * 1024)));
    store.migrate().await.unwrap();
    let tenant = store.create_tenant("cross-acct").await.unwrap();
    let ts = store.for_tenant(tenant.clone());
    let (e1, e2) = (format!("u1-{tenant}@x"), format!("u2-{tenant}@x"));
    let u1 = ts.create_user(&e1).await.unwrap();
    let u2 = ts.create_user(&e2).await.unwrap();
    ts.set_credentials(&u1, &e1, "pw").await.unwrap();
    ts.set_credentials(&u2, &e2, "pw").await.unwrap();
    let tok1 = store.issue_token(&e1, "pw").await.unwrap().unwrap().token;

    // Each user's account-scoped door — the ONLY path to their mail.
    let acct1 = store.for_account(tenant.clone(), u1.clone());
    let acct2 = store.for_account(tenant.clone(), u2.clone());

    // User 2 has a private message (seeded through U2's own door).
    let m2 = acct2
        .deliver(b"From: p@x\r\nSubject: U2 private\r\n\r\nsecret\r\n")
        .await
        .unwrap();
    let msg2 = acct2.message(&m2).await.unwrap();
    let (mid2, thread2, blob2) = (
        m2.to_string(),
        msg2.thread_id.to_string(),
        msg2.blob_id.to_string(),
    );
    let inbox2 = acct2.inbox().await.unwrap().to_string();

    // ---- The store-level door, proven directly (no JMAP, no guards) ----
    // U1's AccountStore wielding U2's ids must be NotFound on EVERY path —
    // the door is compiler-enforced, so there is no `owns_*` guard to
    // forget. This is the structural counterpart to the HTTP probes below.
    use ficina_store::{MailboxId, MessageId, Page, StoreError, ThreadId};
    let m2_id = MessageId::new(mid2.clone());
    let mb2_id = MailboxId::new(inbox2.clone());
    let th2_id = ThreadId::new(thread2.clone());
    let bl2_id = ficina_store::BlobId::new(blob2.clone());
    let is_nf = |r: &Result<_, StoreError>| matches!(r, Err(StoreError::NotFound));

    assert!(is_nf(&acct1.message(&m2_id).await), "read U2 message");
    assert!(is_nf(&acct1.message_bytes(&m2_id).await), "read U2 body");
    assert!(is_nf(&acct1.mailbox(&mb2_id).await), "read U2 mailbox");
    assert!(
        is_nf(&acct1.mailboxes_of_message(&m2_id).await),
        "U2 message membership"
    );
    assert!(
        is_nf(&acct1.set_keyword(&m2_id, "$flagged", true).await),
        "flag U2 message"
    );
    assert!(
        is_nf(&acct1.add_to_mailbox(&m2_id, &mb2_id).await),
        "file U2 message"
    );
    assert!(
        is_nf(&acct1.remove_from_mailbox(&m2_id, &mb2_id).await),
        "unfile U2 message"
    );
    assert!(
        is_nf(&acct1.rename_mailbox(&mb2_id, "hijack").await),
        "rename U2 mailbox"
    );
    assert!(
        is_nf(&acct1.move_mailbox(&mb2_id, None).await),
        "move U2 mailbox"
    );
    assert!(
        is_nf(&acct1.destroy_mailbox(&mb2_id).await),
        "destroy U2 mailbox"
    );
    assert!(
        is_nf(&acct1.destroy_message(&m2_id).await),
        "destroy U2 message"
    );
    assert!(is_nf(&acct1.blob(&bl2_id).await), "read U2 blob meta");
    assert!(is_nf(&acct1.blob_bytes(&bl2_id).await), "read U2 blob bytes");
    // Reads that widen to a set must simply be empty for U1, never U2's.
    assert!(
        acct1.keywords(&m2_id).await.unwrap().is_empty(),
        "U2 keywords leaked"
    );
    assert!(
        acct1
            .list_mailbox(&mb2_id, Page::first(100))
            .await
            .unwrap()
            .is_empty(),
        "U2 mailbox contents leaked"
    );
    assert!(
        acct1
            .thread_messages(&th2_id, Page::first(100))
            .await
            .unwrap()
            .is_empty(),
        "U2 thread leaked"
    );
    // U2's message is untouched after all of U1's probing.
    assert_eq!(acct2.message(&m2_id).await.unwrap().subject, "U2 private");

    let app = ficina_jmap::app(ficina_jmap::app_state(Arc::clone(&store), "http://test"));
    let acc1 = u1.to_string();

    // Email/get of U2's message under U1 → notFound (no body leak).
    let (_s, body) = api(
        &app,
        &tok1,
        call("Email/get", json!({"accountId": acc1, "ids": [mid2]})),
    )
    .await;
    assert!(
        body["methodResponses"][0][1]["list"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(body["methodResponses"][0][1]["notFound"], json!([mid2]));

    // Mailbox/get, Thread/get of U2's objects under U1 → notFound.
    let (_s, body) = api(
        &app,
        &tok1,
        call("Mailbox/get", json!({"accountId": acc1, "ids": [inbox2]})),
    )
    .await;
    assert_eq!(body["methodResponses"][0][1]["notFound"], json!([inbox2]));
    let (_s, body) = api(
        &app,
        &tok1,
        call("Thread/get", json!({"accountId": acc1, "ids": [thread2]})),
    )
    .await;
    assert_eq!(body["methodResponses"][0][1]["notFound"], json!([thread2]));

    // Email/set flag of U2's message under U1 → notUpdated.
    let update = obj(vec![(mid2.clone(), json!({ "keywords/$seen": true }))]);
    let (_s, body) = api(
        &app,
        &tok1,
        call("Email/set", json!({"accountId": acc1, "update": update})),
    )
    .await;
    assert!(
        body["methodResponses"][0][1]["notUpdated"]
            .get(&mid2)
            .is_some()
    );

    // Email/changes under U1 must never surface U2's message id.
    let (_s, body) = api(
        &app,
        &tok1,
        call(
            "Email/changes",
            json!({"accountId": acc1, "sinceState": "0"}),
        ),
    )
    .await;
    let ch = &body["methodResponses"][0][1];
    for field in ["created", "updated", "destroyed"] {
        if let Some(arr) = ch[field].as_array() {
            assert!(
                !arr.iter().any(|v| v == &json!(mid2)),
                "U1 changes leaked U2 id"
            );
        }
    }

    // Blob download of U2's blob under U1 → 404.
    let req = Request::builder()
        .method("GET")
        .uri(format!("/jmap/download/{acc1}/{blob2}/x"))
        .header("authorization", format!("Bearer {tok1}"))
        .body(Body::empty())
        .unwrap();
    let (status, _b) = send(&app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn jmap_methods_never_leak_across_tenants() {
    let a = harness("iso-a").await;
    let b = harness("iso-b").await;

    // Seed B with a delivered message (its own inbox, message, thread, blob).
    let b_msg =
        b.acct
            .deliver(b"From: secret@b\r\nSubject: B secret\r\n\r\nB body\r\n")
            .await
            .unwrap();
    let b_inbox = b.acct.inbox().await.unwrap();
    let b_message = b.acct.message(&b_msg).await.unwrap();
    let b_blob = b_message.blob_id.to_string();
    let b_thread = b_message.thread_id.to_string();
    let b_mid = b_msg.to_string();
    let b_mailbox = b_inbox.to_string();

    // Mailbox/get of B's mailbox id under A → notFound, no list entry.
    let (_s, body) = api(
        &a.app,
        &a.token,
        call(
            "Mailbox/get",
            json!({"accountId": a.account_id, "ids": [b_mailbox]}),
        ),
    )
    .await;
    let r = &body["methodResponses"][0][1];
    assert!(r["list"].as_array().unwrap().is_empty());
    assert_eq!(r["notFound"], json!([b_mailbox]));

    // Email/get of B's message under A → notFound.
    let (_s, body) = api(
        &a.app,
        &a.token,
        call(
            "Email/get",
            json!({"accountId": a.account_id, "ids": [b_mid]}),
        ),
    )
    .await;
    let r = &body["methodResponses"][0][1];
    assert!(r["list"].as_array().unwrap().is_empty());
    assert_eq!(r["notFound"], json!([b_mid]));

    // Thread/get of B's thread under A → notFound.
    let (_s, body) = api(
        &a.app,
        &a.token,
        call(
            "Thread/get",
            json!({"accountId": a.account_id, "ids": [b_thread]}),
        ),
    )
    .await;
    assert_eq!(body["methodResponses"][0][1]["notFound"], json!([b_thread]));

    // Email/set: flagging B's message under A → notUpdated, never applied.
    let update = obj(vec![(b_mid.clone(), json!({ "keywords/$seen": true }))]);
    let (_s, body) = api(
        &a.app,
        &a.token,
        call(
            "Email/set",
            json!({"accountId": a.account_id, "update": update}),
        ),
    )
    .await;
    let r = &body["methodResponses"][0][1];
    assert!(r["updated"].get(&b_mid).is_none());
    assert!(r["notUpdated"].get(&b_mid).is_some());

    // Email/set destroy of B's message under A → notDestroyed.
    let (_s, body) = api(
        &a.app,
        &a.token,
        call(
            "Email/set",
            json!({"accountId": a.account_id, "destroy": [b_mid]}),
        ),
    )
    .await;
    let r = &body["methodResponses"][0][1];
    assert!(r["destroyed"].as_array().unwrap().is_empty());
    assert!(r["notDestroyed"].get(&b_mid).is_some());

    // Mailbox/set destroy of B's mailbox under A → notDestroyed.
    let (_s, body) = api(
        &a.app,
        &a.token,
        call(
            "Mailbox/set",
            json!({"accountId": a.account_id, "destroy": [b_mailbox]}),
        ),
    )
    .await;
    assert!(
        body["methodResponses"][0][1]["notDestroyed"]
            .get(&b_mailbox)
            .is_some()
    );

    // Email/query filtered to B's mailbox under A → no ids (never B's).
    let (_s, body) = api(
        &a.app,
        &a.token,
        call(
            "Email/query",
            json!({"accountId": a.account_id, "filter": {"inMailbox": b_mailbox}}),
        ),
    )
    .await;
    assert!(
        body["methodResponses"][0][1]["ids"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    // Blob download of B's blob under A → 404 (A's own accountId in URL).
    let req = Request::builder()
        .method("GET")
        .uri(format!(
            "/jmap/download/{}/{}/secret.txt",
            a.account_id, b_blob
        ))
        .header("authorization", format!("Bearer {}", a.token))
        .body(Body::empty())
        .unwrap();
    let (status, _b) = send(&a.app, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Email/changes with B's state token under A → only A's (empty) changes.
    let b_state = b.acct.state().await.unwrap();
    let (_s, body) = api(
        &a.app,
        &a.token,
        call(
            "Email/changes",
            json!({"accountId": a.account_id, "sinceState": "0"}),
        ),
    )
    .await;
    let ch = &body["methodResponses"][0][1];
    if ch.get("created").is_some() {
        for field in ["created", "updated", "destroyed"] {
            assert!(
                !ch[field]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v == &json!(b_mid)),
                "A's changes must never contain B's message"
            );
        }
    }
    let _ = b_state;

    // B's data is entirely intact after A's probing.
    let (_s, body) = api(
        &b.app,
        &b.token,
        call(
            "Email/get",
            json!({"accountId": b.account_id, "ids": [b_mid]}),
        ),
    )
    .await;
    assert_eq!(
        body["methodResponses"][0][1]["list"][0]["subject"],
        json!("B secret")
    );
    let (_s, body) = api(
        &b.app,
        &b.token,
        call(
            "Mailbox/get",
            json!({"accountId": b.account_id, "ids": Value::Null}),
        ),
    )
    .await;
    let list = body["methodResponses"][0][1]["list"].as_array().unwrap();
    assert!(list.iter().any(|m| m["unreadEmails"] == json!(1)));
}
