//! In-process integration tests of the public serving surface: real fixtures
//! written through the real store into the compose Postgres, real requests
//! through the real router (`tower::ServiceExt::oneshot`). The Host-isolation
//! test — site A's host can never serve site B's content — is the mandatory
//! one (`docs/design/sites.md`, Tenancy); the rest pin the response contract
//! (cache headers, 304s, the two 404s, method policy) the deploy will rely on.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use serde_json::json;
use tower::ServiceExt;

use alo_sites::serve::{AppState, app};
use alo_store::{AccountStore, BlobStore, SiteId, SitePublicStore, Store};

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
    let store = Store::new(pool.clone(), BlobStore::in_memory(25 * 1024 * 1024));
    store.migrate().await.expect("run migrations");
    let state = AppState::new(SitePublicStore::new(pool), APEX.to_owned());
    (store, state)
}

/// A fresh tenant's account door.
async fn fresh_account(store: &Store, tag: &str) -> AccountStore {
    let tenant = store.create_tenant(&format!("t-{tag}")).await.unwrap();
    let user = store
        .for_tenant(tenant.clone())
        .create_user(&format!("{tag}@serve.test"))
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

/// Creates a site with a home page carrying `marker` and an `/about` page,
/// publishes it, and returns its id.
async fn publish_site(acc: &AccountStore, name: &str, sub: &str, marker: &str) -> SiteId {
    let site = acc.create_site(name, sub).await.unwrap();
    let home = acc.create_site_page(&site, "Home", "", true).await.unwrap();
    acc.set_page_sections(
        &site,
        &home,
        json!({
            "schema_version": 1,
            "sections": [{"type": "hero", "heading": marker}]
        }),
    )
    .await
    .unwrap();
    acc.create_site_page(&site, "About", "about", false)
        .await
        .unwrap();
    acc.publish_site(&site).await.unwrap();
    site
}

/// One in-process request through the real router.
async fn send(state: &Arc<AppState>, host: &str, path: &str) -> Response {
    let request = Request::builder()
        .uri(path)
        .header(header::HOST, host)
        .body(Body::empty())
        .unwrap();
    app(Arc::clone(state)).oneshot(request).await.unwrap()
}

async fn body_string(response: Response) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn header_str<'a>(response: &'a Response, name: &header::HeaderName) -> &'a str {
    response
        .headers()
        .get(name)
        .map(|value| value.to_str().unwrap())
        .unwrap_or("")
}

#[tokio::test]
async fn serves_a_published_site_with_the_response_contract() {
    let (store, state) = harness().await;
    let acc = fresh_account(&store, "serve").await;
    let sub = unique("serve");
    publish_site(&acc, "Contract Co", &sub, "CONTRACT-MARKER").await;
    let host = format!("{sub}.{APEX}");

    // Liveness, host-independent.
    let health = send(&state, "anything", "/healthz").await;
    assert_eq!(health.status(), StatusCode::OK);
    assert_eq!(body_string(health).await, "ok\n");

    // The home page, with the full header contract.
    let home = send(&state, &host, "/").await;
    assert_eq!(home.status(), StatusCode::OK);
    assert_eq!(
        header_str(&home, &header::CONTENT_TYPE),
        "text/html; charset=utf-8"
    );
    assert_eq!(
        header_str(&home, &header::CACHE_CONTROL),
        "public, max-age=60"
    );
    assert_eq!(
        header_str(&home, &header::X_CONTENT_TYPE_OPTIONS),
        "nosniff"
    );
    let etag = header_str(&home, &header::ETAG).to_owned();
    assert!(etag.starts_with('"') && etag.ends_with('"'), "strong ETag");
    let html = body_string(home).await;
    assert!(html.contains("CONTRACT-MARKER"));
    assert!(html.contains("Contract Co"));
    assert!(
        html.contains(&format!("https://{host}/")),
        "canonical URL is built from the Host"
    );

    // A port on the Host header must not change resolution.
    let with_port = send(&state, &format!("{host}:8081"), "/").await;
    assert_eq!(with_port.status(), StatusCode::OK);

    // Conditional revalidation: the ETag round-trips as a 304.
    let request = Request::builder()
        .uri("/")
        .header(header::HOST, &host)
        .header(header::IF_NONE_MATCH, &etag)
        .body(Body::empty())
        .unwrap();
    let revalidated = app(Arc::clone(&state)).oneshot(request).await.unwrap();
    assert_eq!(revalidated.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(header_str(&revalidated, &header::ETAG), etag);
    assert_eq!(body_string(revalidated).await, "");

    // Subpage, and the trailing-slash variant serving the same document.
    let about = body_string(send(&state, &host, "/about").await).await;
    let about_slash = body_string(send(&state, &host, "/about/").await).await;
    assert!(about.contains("About"));
    assert_eq!(about, about_slash);

    // The one stylesheet.
    let css = send(&state, &host, "/assets/site.css").await;
    assert_eq!(css.status(), StatusCode::OK);
    assert_eq!(
        header_str(&css, &header::CONTENT_TYPE),
        "text/css; charset=utf-8"
    );
    assert!(body_string(css).await.contains(":root"));

    // An unknown path on a live site: the site's themed 404.
    let missing = send(&state, &host, "/no-such-page").await;
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(header_str(&missing, &header::CACHE_CONTROL), "no-cache");
    let missing_html = body_string(missing).await;
    assert!(
        missing_html.contains("Contract Co"),
        "404 stays in the brand"
    );
    assert!(missing_html.contains("Page not found"));

    // Anything but GET/HEAD is refused with the allow list.
    let post = Request::builder()
        .method("POST")
        .uri("/")
        .header(header::HOST, &host)
        .body(Body::empty())
        .unwrap();
    let refused = app(Arc::clone(&state)).oneshot(post).await.unwrap();
    assert_eq!(refused.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(header_str(&refused, &header::ALLOW), "GET, HEAD");
}

#[tokio::test]
async fn host_isolation_one_host_never_serves_another_sites_content() {
    let (store, state) = harness().await;
    let a = fresh_account(&store, "iso-a").await;
    let b = fresh_account(&store, "iso-b").await;
    let sub_a = unique("iso-a");
    let sub_b = unique("iso-b");
    publish_site(&a, "Alpha Site", &sub_a, "ALPHA-ONLY").await;
    publish_site(&b, "Beta Site", &sub_b, "BETA-ONLY").await;

    // Each host serves exactly its own tenant's content.
    let alpha = body_string(send(&state, &format!("{sub_a}.{APEX}"), "/").await).await;
    let beta = body_string(send(&state, &format!("{sub_b}.{APEX}"), "/").await).await;
    assert!(alpha.contains("ALPHA-ONLY") && !alpha.contains("BETA-ONLY"));
    assert!(beta.contains("BETA-ONLY") && !beta.contains("ALPHA-ONLY"));

    // A's host cannot reach B's stylesheet-styling theme or pages either:
    // the whole render under A's host is A's publish, so B's markers can
    // appear nowhere in any response for A's host.
    let alpha_css =
        body_string(send(&state, &format!("{sub_a}.{APEX}"), "/assets/site.css").await).await;
    assert!(!alpha_css.contains("BETA"));
    let cross = send(&state, &format!("{sub_a}.{APEX}"), "/beta-page").await;
    assert_eq!(cross.status(), StatusCode::NOT_FOUND);
    let cross_html = body_string(cross).await;
    assert!(!cross_html.contains("BETA-ONLY") && !cross_html.contains("Beta Site"));
}

#[tokio::test]
async fn unknown_and_unpublished_hosts_are_indistinguishable() {
    let (store, state) = harness().await;
    let acc = fresh_account(&store, "leak").await;
    let sub_created = unique("leak-created");
    // A site that exists but was never published.
    acc.create_site("Hidden Co", &sub_created).await.unwrap();

    let unknown = send(&state, &format!("{}.{APEX}", unique("leak-none")), "/").await;
    let unpublished = send(&state, &format!("{sub_created}.{APEX}"), "/").await;
    assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
    assert_eq!(unpublished.status(), StatusCode::NOT_FOUND);
    let unknown_body = body_string(unknown).await;
    let unpublished_body = body_string(unpublished).await;
    assert_eq!(
        unknown_body, unpublished_body,
        "no existence leak: unknown and unpublished serve identical bytes"
    );
    assert!(!unpublished_body.contains("Hidden Co"));

    // Hosts that resolve to no subdomain at all get the same body too.
    for host in [
        APEX,
        &format!("a.b.{APEX}"),
        "example.com",
        &format!("x{APEX}"),
    ] {
        let response = send(&state, host, "/").await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "host {host}");
        assert_eq!(body_string(response).await, unknown_body, "host {host}");
    }
}

#[tokio::test]
async fn republish_flips_content_immediately_and_drafts_never_leak() {
    let (store, state) = harness().await;
    let acc = fresh_account(&store, "flip").await;
    let sub = unique("flip");
    let host = format!("{sub}.{APEX}");
    let site = publish_site(&acc, "Flip Co", &sub, "VERSION-ONE").await;

    // Serve once (fills the cache), then edit the draft: the served bytes
    // must not move — the publish is frozen and the cache is keyed by it.
    assert!(
        body_string(send(&state, &host, "/").await)
            .await
            .contains("VERSION-ONE")
    );
    let pages = acc.site_pages(&site).await.unwrap();
    let home = pages.iter().find(|p| p.is_home).unwrap();
    acc.set_page_sections(
        &site,
        &home.id,
        json!({
            "schema_version": 1,
            "sections": [{"type": "hero", "heading": "VERSION-TWO"}]
        }),
    )
    .await
    .unwrap();
    let still = body_string(send(&state, &host, "/").await).await;
    assert!(still.contains("VERSION-ONE") && !still.contains("VERSION-TWO"));

    // Republish: the very next request serves the new set (no TTL lag —
    // the resolver read runs per request and the publish id flipped).
    acc.publish_site(&site).await.unwrap();
    let flipped = body_string(send(&state, &host, "/").await).await;
    assert!(flipped.contains("VERSION-TWO") && !flipped.contains("VERSION-ONE"));

    // Unpublish: immediately back to the generic not-found.
    acc.unpublish_site(&site).await.unwrap();
    let gone = send(&state, &host, "/").await;
    assert_eq!(gone.status(), StatusCode::NOT_FOUND);
    assert!(!body_string(gone).await.contains("VERSION-TWO"));
}
