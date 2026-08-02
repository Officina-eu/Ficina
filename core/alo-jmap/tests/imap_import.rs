//! IMAP import: the dedup/ingest half against a real store (import a
//! batch, re-import it → all skipped, verify the new mail lands in the
//! Inbox and stays tenant-scoped), plus the endpoint's guard rails —
//! validation and the SSRF refusal of a private/loopback host.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use alo_jmap::imap_import::{ImportOutcome, import_messages};
use alo_store::Page;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{harness, send};

fn msg(id: &str, subject: &str) -> Vec<u8> {
    format!(
        "From: old@example.eu\r\nTo: me@example.test\r\nSubject: {subject}\r\n\
         Message-ID: <{id}>\r\nDate: Mon, 27 Jul 2026 00:00:00 +0000\r\n\r\nbody {id}\r\n"
    )
    .into_bytes()
}

#[tokio::test]
async fn import_ingests_then_dedupes_on_reimport() {
    let h = harness("imap-dedup").await;
    let inbox = h.acc.inbox().await.unwrap();
    let before = h
        .acc
        .list_mailbox(&inbox, Page::default())
        .await
        .unwrap()
        .len();

    let batch = vec![msg("a@imp", "first"), msg("b@imp", "second")];

    // First import: both are new.
    let out = import_messages(&h.acc, batch.clone()).await.unwrap();
    assert_eq!(
        out,
        ImportOutcome {
            imported: 2,
            skipped: 0,
            failed: 0
        }
    );
    let after = h.acc.list_mailbox(&inbox, Page::default()).await.unwrap();
    assert_eq!(after.len(), before + 2, "both landed in the Inbox");
    assert!(after.iter().any(|m| m.subject == "first"));

    // Re-import the same batch: both already present → skipped, none added.
    let again = import_messages(&h.acc, batch).await.unwrap();
    assert_eq!(
        again,
        ImportOutcome {
            imported: 0,
            skipped: 2,
            failed: 0
        }
    );
    assert_eq!(
        h.acc
            .list_mailbox(&inbox, Page::default())
            .await
            .unwrap()
            .len(),
        before + 2,
        "no duplicates on re-import"
    );
}

#[tokio::test]
async fn imported_mail_is_tenant_scoped() {
    let a = harness("imap-iso-a").await;
    let b = harness("imap-iso-b").await;
    import_messages(&a.acc, vec![msg("secret@imp", "A only")])
        .await
        .unwrap();

    let b_inbox = b.acc.inbox().await.unwrap();
    let b_list = b.acc.list_mailbox(&b_inbox, Page::default()).await.unwrap();
    assert!(
        b_list.iter().all(|m| m.subject != "A only"),
        "B never sees A's imported mail"
    );
}

async fn post_import(h: &common::Harness, body: &str) -> (StatusCode, String) {
    let req = Request::builder()
        .method("POST")
        .uri("/import/imap")
        .header("authorization", format!("Bearer {}", h.token))
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap();
    let (status, json) = send(&h.app, req).await;
    (status, json.to_string())
}

#[tokio::test]
async fn endpoint_validates_and_refuses_ssrf() {
    let h = harness("imap-guard").await;

    // Missing fields → 400.
    let (status, _) = post_import(&h, r#"{"host":"","username":"","password":""}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // A loopback/private host is refused by the SSRF guard (Host → 400),
    // never dialed — the import wizard must not become an internal-network
    // probe. 127.0.0.1 resolves to a blocked address.
    let (status, _) = post_import(
        &h,
        r#"{"host":"127.0.0.1","port":993,"username":"u","password":"p"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "loopback host refused");
}
