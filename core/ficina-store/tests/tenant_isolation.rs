//! The wrong-tenant suite (mandatory, CI-gated). For **every** public
//! read and write path, tenant A addressing tenant B's ids must get a
//! clean `NotFound`/empty — never B's data, never an unexpected error
//! (a "500"). Runs against the real Postgres from compose.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use ficina_store::{Page, SEEN, StoreError};

/// Asserts a result is the clean not-found denial — never data, never an
/// internal (`Db`/`Blob`/`Migrate`) error.
fn assert_not_found<T: std::fmt::Debug>(result: Result<T, StoreError>) {
    match result {
        Err(StoreError::NotFound) => {}
        Err(other) => panic!("expected NotFound, got internal error: {other:?}"),
        Ok(value) => panic!("expected NotFound, but got data: {value:?}"),
    }
}

#[tokio::test]
async fn wrong_tenant_reads_and_writes_are_denied_never_leaked() {
    let store = common::test_store().await;
    let a = common::tenant_fixture(&store, "iso-a").await;
    let b = common::tenant_fixture(&store, "iso-b").await;

    // --- reads that must return NotFound (single-row lookups) ---
    assert_not_found(a.ts.mailbox(&b.inbox).await);
    assert_not_found(a.ts.message(&b.message).await);
    assert_not_found(a.ts.message_bytes(&b.message).await);

    // --- reads that must return EMPTY (list/collection paths) — never
    //     the other tenant's rows ---
    assert!(
        a.ts.list_mailbox(&b.inbox, Page::default())
            .await
            .unwrap()
            .is_empty(),
        "A must not list B's mailbox contents"
    );
    assert!(
        a.ts.keywords(&b.message).await.unwrap().is_empty(),
        "A must not read B's keywords"
    );
    assert!(
        a.ts.mailboxes_for_user(&b.user, Page::default())
            .await
            .unwrap()
            .is_empty(),
        "A must not list B's mailboxes"
    );
    assert!(
        a.ts.search(&b.user, "body", Page::default())
            .await
            .unwrap()
            .is_empty(),
        "A must not search B's messages"
    );
    let b_thread = b.ts.message(&b.message).await.unwrap().thread_id;
    assert!(
        a.ts.thread_messages(&b_thread, Page::default())
            .await
            .unwrap()
            .is_empty(),
        "A cannot enumerate B's thread"
    );

    // --- writes that must return NotFound (foreign id on a write) ---
    assert_not_found(a.ts.set_keyword(&b.message, SEEN, true).await);
    assert_not_found(a.ts.add_to_mailbox(&b.message, &a.inbox).await); // foreign message
    assert_not_found(a.ts.add_to_mailbox(&a.message, &b.inbox).await); // foreign mailbox
    assert_not_found(a.ts.remove_from_mailbox(&b.message, &a.inbox).await);
    assert_not_found(a.ts.create_mailbox(&b.user, None, "Evil", None).await);
    assert_not_found(a.ts.inbox(&b.user).await);
    assert_not_found(a.ts.ingest(&b.user, &a.inbox, b"From: x\r\n\r\nx").await);
    assert_not_found(a.ts.ingest(&a.user, &b.inbox, b"From: x\r\n\r\nx").await);
    assert_not_found(a.ts.deliver(&b.user, b"From: x\r\n\r\nx").await);

    // --- B's data is completely intact after A's probing ---
    let b_inbox = b.ts.mailbox(&b.inbox).await.unwrap();
    assert_eq!(b_inbox.total_messages, 1, "B still has its one message");
    assert_eq!(b_inbox.unread_messages, 1);
    assert_eq!(
        b.ts.list_mailbox(&b.inbox, Page::default())
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(b.ts.message(&b.message).await.is_ok());
    // And A only ever sees its own single message.
    let a_list = a.ts.list_mailbox(&a.inbox, Page::default()).await.unwrap();
    assert_eq!(a_list.len(), 1);
    assert_eq!(a_list[0].id, a.message);
}

#[tokio::test]
async fn blobs_do_not_leak_across_tenants_even_at_identical_content() {
    // Two tenants deliver byte-identical messages: the content hash is the
    // same, but each tenant reads only under its own key prefix, and one
    // cannot read the other's message bytes.
    let store = common::test_store().await;
    let ta = store.create_tenant("blob-a").await.unwrap();
    let tb = store.create_tenant("blob-b").await.unwrap();
    let a = store.for_tenant(ta);
    let b = store.for_tenant(tb);
    let ua = a.create_user("a@example.test").await.unwrap();
    let ub = b.create_user("b@example.test").await.unwrap();
    let raw = b"From: same@example.test\r\nSubject: identical\r\n\r\nidentical body\r\n";
    let ma = a.deliver(&ua, raw).await.unwrap();
    let mb = b.deliver(&ub, raw).await.unwrap();

    assert_eq!(a.message_bytes(&ma).await.unwrap().as_ref(), raw);
    assert_eq!(b.message_bytes(&mb).await.unwrap().as_ref(), raw);
    // A cannot read B's message even though the bytes are identical.
    assert_not_found(a.message_bytes(&mb).await);
    assert_not_found(b.message_bytes(&ma).await);
}
