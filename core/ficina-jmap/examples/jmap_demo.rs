//! A demo JMAP server for manual/curl evidence. Seeds an idempotent
//! `demo@ficina.test` account (password `demo-pass`) with one message,
//! then serves on 127.0.0.1:8090.
//!
//! Run: `DATABASE_URL=... cargo run -p ficina-jmap --example serve_demo`
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use ficina_identity::{Identity, IdentityConfig};
use ficina_store::{BlobStore, Store};

#[tokio::main]
async fn main() {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL");
    let store = Arc::new(
        Store::connect(&url, BlobStore::in_memory(50 * 1024 * 1024))
            .await
            .expect("connect"),
    );
    store.migrate().await.expect("migrate");
    let identity = Identity::new(
        Arc::clone(&store),
        IdentityConfig::new("http://127.0.0.1:8090"),
    )
    .expect("identity");

    let email = "demo@ficina.test";
    if identity
        .password_login(email, "demo-pass", None)
        .await
        .expect("login")
        .is_none()
    {
        let tenant = store.create_tenant("demo").await.unwrap();
        let ts = store.for_tenant(tenant.clone());
        let user = ts.create_user(email).await.unwrap();
        identity
            .set_password(&tenant, &user, email, "demo-pass")
            .await
            .unwrap();
        let acc = store.for_account(tenant, user);
        acc.deliver(
            b"From: Alice <alice@example.com>\r\nTo: demo@ficina.test\r\n\
              Subject: Welcome to Ficina\r\nMessage-ID: <welcome@ficina.test>\r\n\r\n\
              Hello from the JMAP API.\r\n",
        )
        .await
        .unwrap();
    }

    let addr = "127.0.0.1:8090".parse().unwrap();
    let state = ficina_jmap::app_state(Arc::clone(&store), identity, "http://127.0.0.1:8090");
    println!("ficina-jmap demo on http://127.0.0.1:8090 (demo@ficina.test / demo-pass)");
    ficina_jmap::serve(addr, state).await.unwrap();
}
