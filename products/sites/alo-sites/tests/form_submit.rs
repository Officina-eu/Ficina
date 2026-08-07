//! In-process integration tests of `POST /f/:form_id` (`docs/design/
//! sites.md`, form flow): real fixtures through the real store into the
//! compose Postgres, real requests through the real router. The mandatory
//! isolation case is the foreign/unknown form id yielding one clean 404 —
//! indistinguishable for an id that never existed and a form on a draft
//! site — plus proof that an accepted submission lands only in the owning
//! tenant. The rest pin the wire contract: honeypot silent drop, 400 on
//! malformed or invalid input, 413 on an oversized body, 429 with
//! `Retry-After` under the per-client rate limit.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use tower::ServiceExt;

use alo_sites::serve::{AppState, app};
use alo_store::{AccountStore, BlobStore, SiteFormId, SiteId, SitePublicStore, Store};

/// The apex the tests serve under (`SITES_DOMAIN` in production).
const APEX: &str = "sites.test";

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://alo:alo-dev-only@127.0.0.1:5433/alo".to_owned())
}

/// A migrated store plus the service state sharing the same Postgres.
async fn harness() -> (Store, Arc<AppState>) {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(6)
        .connect(&database_url())
        .await
        .expect("connect to test postgres (is compose up? is DATABASE_URL set?)");
    let blobs = BlobStore::in_memory(25 * 1024 * 1024);
    let store = Store::new(pool.clone(), blobs.clone());
    store.migrate().await.expect("run migrations");
    let state = AppState::new(SitePublicStore::new(pool, blobs), APEX.to_owned());
    (store, state)
}

/// A fresh tenant's account door.
async fn fresh_account(store: &Store, tag: &str) -> AccountStore {
    let tenant = store.create_tenant(&format!("t-{tag}")).await.unwrap();
    let user = store
        .for_tenant(tenant.clone())
        .create_user(&format!("{tag}@forms.test"))
        .await
        .unwrap();
    store.for_account(tenant, user)
}

/// A subdomain that is unique per test run (the Postgres is shared).
fn unique(tag: &str) -> String {
    format!(
        "{tag}-{}x",
        SiteId::generate().as_str().to_lowercase().replace('_', "-")
    )
}

/// A live site (one published home page) with one contact form on it.
async fn live_site_with_form(acc: &AccountStore, tag: &str) -> (SiteId, SiteFormId) {
    let site = acc.create_site("Form Co", &unique(tag)).await.unwrap();
    acc.create_site_page(&site, "Home", "", true).await.unwrap();
    acc.publish_site(&site).await.unwrap();
    let form = acc.create_site_form(&site, "Contact").await.unwrap();
    (site, form)
}

/// One urlencoded submission POST through the real router. `client` becomes
/// the `X-Forwarded-For` value — how tests model distinct senders.
async fn post_form(state: &Arc<AppState>, form_id: &str, client: &str, body: &str) -> Response {
    let request = Request::builder()
        .method("POST")
        .uri(format!("/f/{form_id}"))
        .header(header::HOST, format!("whatever.{APEX}"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header("x-forwarded-for", client)
        .body(Body::from(body.to_owned()))
        .unwrap();
    app(Arc::clone(state)).oneshot(request).await.unwrap()
}

async fn body_string(response: Response) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn accepts_a_valid_submission_into_the_owning_tenant_only() {
    let (store, state) = harness().await;
    let owner = fresh_account(&store, "accept").await;
    let outsider = fresh_account(&store, "accept-outsider").await;
    let (site, form) = live_site_with_form(&owner, "accept").await;

    let response = post_form(
        &state,
        form.as_str(),
        "203.0.113.1",
        "name=Ada+Lovelace&email=ada%40example.test&message=Hello+there&website=",
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let page = body_string(response).await;
    assert!(page.contains("Message sent"), "success page, got: {page}");

    let stored = owner.site_form_submissions(&site, &form).await.unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].sender_name, "Ada Lovelace");
    assert_eq!(stored[0].sender_email, "ada@example.test");
    assert_eq!(stored[0].message, "Hello there");
    assert!(!stored[0].handled);

    // The row is in the owner's tenant and nowhere else: the outsider's
    // door sees nothing through any address for it.
    assert!(
        outsider
            .site_form_submissions(&site, &form)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn unknown_foreign_and_draft_site_form_ids_are_one_clean_404() {
    let (store, state) = harness().await;
    let owner = fresh_account(&store, "missing").await;
    // A real form — but its site was never published.
    let draft_site = owner
        .create_site("Draft Co", &unique("missing"))
        .await
        .unwrap();
    let draft_form = owner
        .create_site_form(&draft_site, "Contact")
        .await
        .unwrap();

    let valid = "name=Eve&email=eve%40example.test&message=knock";
    for form_id in [SiteFormId::generate().as_str(), draft_form.as_str()] {
        let response = post_form(&state, form_id, "203.0.113.2", valid).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "id {form_id}");
    }
    assert!(
        owner
            .site_form_submissions(&draft_site, &draft_form)
            .await
            .unwrap()
            .is_empty(),
        "a 404 must write nothing"
    );
}

#[tokio::test]
async fn honeypot_is_a_silent_drop_indistinguishable_from_success() {
    let (store, state) = harness().await;
    let owner = fresh_account(&store, "honeypot").await;
    let (site, form) = live_site_with_form(&owner, "honeypot").await;

    let response = post_form(
        &state,
        form.as_str(),
        "203.0.113.3",
        "name=Bot&email=bot%40example.test&message=spam&website=https%3A%2F%2Fspam.example",
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK, "the bot sees success");
    assert!(body_string(response).await.contains("Message sent"));
    assert!(
        owner
            .site_form_submissions(&site, &form)
            .await
            .unwrap()
            .is_empty(),
        "but nothing was written"
    );
}

#[tokio::test]
async fn malformed_and_invalid_bodies_are_400_and_write_nothing() {
    let (store, state) = harness().await;
    let owner = fresh_account(&store, "invalid").await;
    let (site, form) = live_site_with_form(&owner, "invalid").await;

    // Broken percent-encoding: unreadable as a form at all.
    let broken = post_form(&state, form.as_str(), "203.0.113.4", "name=%zz").await;
    assert_eq!(broken.status(), StatusCode::BAD_REQUEST);

    // Readable but failing the write gate: blank name, bad email, blank
    // message — each 400 with the field-level reason on the page.
    for (body, reason) in [
        ("name=&email=a%40b.test&message=hi", "name"),
        ("name=Ada&email=nope&message=hi", "email"),
        ("name=Ada&email=a%40b.test&message=+++", "message"),
    ] {
        let response = post_form(&state, form.as_str(), "203.0.113.4", body).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "body {body}");
        let page = body_string(response).await;
        assert!(page.contains(reason), "page names {reason}, got: {page}");
    }

    assert!(
        owner
            .site_form_submissions(&site, &form)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn an_oversized_body_is_413() {
    let (store, state) = harness().await;
    let owner = fresh_account(&store, "oversize").await;
    let (_, form) = live_site_with_form(&owner, "oversize").await;

    let huge = format!(
        "name=Ada&email=a%40b.test&message={}",
        "x".repeat(300 * 1024)
    );
    let response = post_form(&state, form.as_str(), "203.0.113.5", &huge).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn the_per_client_rate_limit_answers_429_with_retry_after() {
    let (store, state) = harness().await;
    let owner = fresh_account(&store, "rate").await;
    let (site, form) = live_site_with_form(&owner, "rate").await;

    // Exhaust one client's window (honeypot bodies: attempts count against
    // the limiter before anything else, and no rows accumulate).
    let burn = "name=Bot&email=bot%40example.test&message=spam&website=x";
    for n in 0..10 {
        let response = post_form(&state, form.as_str(), "198.51.100.7", burn).await;
        assert_eq!(response.status(), StatusCode::OK, "attempt {n} in budget");
    }
    let limited = post_form(&state, form.as_str(), "198.51.100.7", burn).await;
    assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
    let retry_after = limited
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .expect("Retry-After header in seconds");
    assert!((1..=600).contains(&retry_after));

    // A different client is untouched — and its valid submission lands.
    let other = post_form(
        &state,
        form.as_str(),
        "198.51.100.8",
        "name=Grace&email=grace%40example.test&message=Still+works",
    )
    .await;
    assert_eq!(other.status(), StatusCode::OK);
    let stored = owner.site_form_submissions(&site, &form).await.unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].sender_name, "Grace");
}
