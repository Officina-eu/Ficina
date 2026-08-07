# sites/STATE.md — Sites-track loop journal (append-only; newest at the bottom)

One entry per iteration: item id, what shipped, how verified, cuts/flags,
next item. The end-of-queue / emergency-stop control markers the wrapper
watches for are defined in LOOP.md — never write those exact phrases here
except to actually fire them.

Human-action inbox (things the loop must not do itself):

- ~~Buy/choose the public sites domain and set wildcard DNS~~ **DONE
  2026-08-07: alosites.com purchased (Namecheap). Verified live on public
  DNS: apex/`*`/www A → 152.53.179.142, null SPF + DMARC reject. The env for
  the alo-sites service is `SITES_DOMAIN=alosites.com`.**
- At next deploy: add the alo-sites container to production compose + Caddy
  wildcard/on-demand-TLS config (the loop never touches deploy/).
- Configure an AI provider key on the live server before real "generate my
  site" runs (loop verifies with fixtures only).
- Post-launch hardening (not urgent): submit alosites.com to the Public
  Suffix List so browsers isolate customer subdomains from each other.

---

## S1.01 — design note docs/design/sites.md (2026-08-06)

- **Shipped:** `docs/design/sites.md` — full v1 design: data model (sites,
  site_pages with versioned typed-section JSON envelope, immutable
  site_page_snapshots, themes, site_posts, site_forms/submissions,
  site_domains, site_analytics_daily), render pipeline (pure render lib
  shared by alo-sites public serving and the alo-jmap draft preview),
  two-service boundary, form flow (honeypot/rate-limit/internal-delivery),
  privacy analytics model (no-PII schema asserted by test), tenancy model
  incl. the two deliberate global surfaces (subdomain unique index,
  host→site resolver), error maps for both services, and out-of-scope list.
  Both required rejected alternatives recorded: free-form canvas vs typed
  sections, and public serving inside alo-jmap vs a separate binary.
- **Verified:** docs-only item — no code gates apply; note answers all four
  implement-skill blocks (Surface / Errors / Tenancy / Out of scope) plus
  the rejected alternative required by the queue's done-criterion.
- **Cuts/flags:** none. CHANGELOG untouched (no behaviour change).
- **Next:** S1.02 (sites migration + store + wrong-tenant tests).

## S1.02 — sites migration + store module (2026-08-06)

- **Shipped:** migration `0055_sites.sql` (tenant-scoped `sites` table,
  tenant-cascade FK, and the one deliberate global surface: a cross-tenant
  unique index on `subdomain`); new `SiteId`; `platform/alo-store/src/sites.rs`
  on the account door — `create_site` / `sites` / `site` / `rename_site` /
  `set_site_subdomain` / `delete_site` / `subdomain_available`, with subdomain
  validation (DNS-safe `[a-z0-9-]{3,40}`, no edge hyphens, ~80-word reserved
  list covering infrastructure/mail/identity/brand labels) and a
  `SiteStatus` draft/live enum. Unique-violation on the subdomain index maps
  to `Conflict("subdomain is already taken")` — taken/free only, never owner.
- **Verified:** `cargo fmt`; `SQLX_OFFLINE=true cargo clippy -p alo-store
  --all-targets` zero warnings; full `cargo test -p alo-store` green against
  the local docker Postgres (migration really applied — `\d sites` shows the
  table, PK, global unique index, cascade FK, and rows from the test run).
  New tests: 5 unit tests on validation/status tokens (incl. one asserting
  every reserved-list entry passes the syntax rules — caught dead-weight
  `mx`), plus `sites_scope_by_tenant_and_subdomains_are_globally_unique` in
  the isolation suite: outsider tenant gets clean `NotFound` on every path,
  co-tenant user shares the sites, cross-tenant claim collides with a
  taken-only message, delete releases the subdomain.
- **Cuts/flags:**
  - No theme setter yet — the column ships with `'{}'` default; a raw
    unvalidated write surface would predate the typed theme model, so the
    setter lands with S1.05.
  - No status setter — `live` is the publish flow's to flip (S1.08).
  - Drive-by fix (out of item scope but blocking the mandatory gate): the
    pre-existing isolation test `deleting_a_tenant_purges_its_tasks` was
    red on main — `task_projects()`'s lazy `ensure_personal_project()`
    INSERT hits the tenant FK after tenant deletion and surfaced as a `Db`
    error. `tasks.rs` now treats that FK violation (SQLSTATE 23503) as
    "nothing to ensure", so a deleted tenant reads as absent, never a 500.
  - `cargo fmt -p alo-store` also normalized previously unformatted code in
    `base.rs`, `tasks.rs`, and older `tenant_isolation.rs` tests —
    mechanical churn, kept so the crate is fmt-clean.
  - CHANGELOG untouched: storage foundation only, no user-visible behaviour.
- **Next:** S1.03 (typed section schema v1 in a `site_model` module).

## S1.03 — typed section schema v1, `site_model` (2026-08-06)

- **Shipped:** `platform/alo-store/src/site_model.rs` — the closed v1 section
  vocabulary as an internally-tagged serde enum (`type` tag, snake_case):
  nav, hero, features, text_image, gallery, testimonials, pricing, team,
  faq, cta, contact_form, footer, each with typed props and
  `deny_unknown_fields`; the `SectionsEnvelope { schema_version, sections }`
  write gate (`from_value` = version check before shape check → strict serde
  parse → content rules); content validation covering text bounds
  (300/5 000 chars), list bounds (≤50, non-empty where meaningless empty),
  href allowlist (`/path`, `#fragment`, http(s)/mailto/tel; rejects
  `javascript:`, `data:`, protocol-relative — stored hrefs are always safe
  in an `href` attribute), blob/form id token shape, and icon token shape.
  Pricing `price` is a display string by design (no money computation —
  integer-cents law not in play, per the design note). Golden fixtures: 12
  per-section envelopes + a full-page fixture with all 12 in order
  (`tests/fixtures/site_sections/`), pinned by `tests/site_sections.rs`
  round-trip-to-identical-Value tests. Enabling change: the `opaque_id!`
  macro now also derives serde Serialize/Deserialize (newtype-transparent,
  purely additive) so `BlobId` can live typed inside section JSON.
- **Verified:** `cargo fmt`; `SQLX_OFFLINE=true cargo clippy -p alo-store
  --all-targets` zero warnings; full `cargo test -p alo-store` green against
  the local docker Postgres (77 unit incl. 10 new schema tests — exhaustive
  fully-populated and minimal round-trips, wire-tag pinning, unknown
  type/prop rejection, future-version error precedence, href/token/content
  rules — plus 3 new golden-fixture tests and the whole isolation suite).
  No storage/routes touched, so wrong-tenant and wire-verify gates don't
  apply; pure model only.
- **Cuts/flags:**
  - Read-side tolerance (skip-with-log on unknown sections so an old
    renderer survives a newer snapshot) deliberately NOT here — it is the
    renderer's job and lands with S1.06, as the design note specifies.
  - `contact_form.form_id` is a plain validated token (`Option<String>`)
    until the forms table + id newtype land in S1.16; wire shape is final.
  - Environment note: parallel rustc runs OOM-killed the first full test
    build on this machine; `cargo test -j 2` builds fine. The DB tests need
    `DATABASE_URL=postgres://alo:alo-dev-only@localhost:5432/alo` (harness
    default points at 5433).
  - CHANGELOG untouched: schema foundation only, no user-visible behaviour.
- **Next:** S1.04 (site_pages migration + store, sections validated through
  this module on every write).

## S1.04 — site_pages migration + store module (2026-08-06)

- **Shipped:** migration `0056_site_pages.sql` (tenant-scoped `site_pages`,
  composite FK cascading tenants → sites → pages, per-site slug unique index,
  partial unique index enforcing one home page per site, and a CHECK that
  only the home page may hold the empty slug); new `SitePageId`;
  `platform/alo-store/src/site_pages.rs` on the account door —
  `create_site_page` (appends at end of nav order, empty sections envelope,
  200-pages-per-site cap) / `site_pages` / `site_page` / `set_page_title` /
  `set_page_slug` / `set_page_seo` (trim, blank-clears, 200/500 char caps) /
  `set_page_sections` (the schema write gate: `SectionsEnvelope::from_value`
  from S1.03, canonical serialization stored) / `set_home_page` (transactional
  demote+promote; demoting a home at the empty slug is a named Conflict) /
  `reorder_site_pages` (full-permutation rewrite in a transaction) /
  `delete_site_page`. Slug rules: `[a-z0-9-]{1,80}`, no edge hyphens,
  reserved public paths (`blog`, `f`, `feed`, `rss`, `atom`, `sitemap`,
  `robots`, `healthz`, `assets`, `static`) rejected; empty slug is the home
  page's spelling, DB-enforced so the rule holds under concurrency. All
  constraint violations map to named `Conflict`s, never a raw 23xxx error.
- **Verified:** `cargo fmt`; `SQLX_OFFLINE=true cargo clippy -p alo-store
  --all-targets` zero warnings; full `cargo test -p alo-store` green on the
  local docker Postgres (82 unit incl. 5 new slug/SEO rule tests; isolation
  suite 22 incl. the new
  `site_pages_scope_by_tenant_and_site_with_slug_and_home_rules` — outsider
  tenant cleanly denied on all ten paths, same-tenant cross-SITE addressing
  denied, per-site slug reuse allowed, home-flag flip + empty-slug demote
  conflict, sections gate accept/reject, reorder permutation checks, delete
  frees slug, site delete cascades pages). Manual pass: `\d site_pages` in
  psql shows the PK, both unique indexes, the CHECK, and the cascade FK
  exactly as designed.
- **Cuts/flags:**
  - `sections` is returned as stored (opaque `Value`) on read — typed
    read-side handling with skip-with-log tolerance is the renderer's job
    (S1.06), per the design note; the write gate guarantees whatever is on
    disk passed the schema.
  - No route/UI surface yet (S1.10/S1.11) — store + tests only, so the
    wire-verify gate doesn't apply; CHANGELOG untouched (no user-visible
    behaviour).
  - Page cap decision: `MAX_PAGES_PER_SITE = 200` (not in the queue text;
    bounded input everywhere — revisit with quotas if it ever binds).
- **Next:** S1.05 (theme model: palette+typography presets + logo/favicon
  blob refs).

## S1.05 — typed theme model + theme setter (2026-08-07)

- **Shipped:** `platform/alo-store/src/site_theme.rs` — the theme envelope
  `SiteTheme { schema_version, preset, logo?, favicon? }` with the same
  gate pattern as S1.03 (`from_value` = version-before-shape strict parse →
  content rules; `deny_unknown_fields`; absent options stored as absent
  keys) plus `from_stored`, the never-fail read spelling that maps the
  pristine `{}` column default to the default theme. Seven shipped presets
  (`north` default, `ink`, `terra`, `fern`, `plum`, `carbon`, `midnight`),
  each palette (7 hex tokens) + typography (system-font stacks ONLY — a
  published site loads no third-party font, that's the privacy promise —
  plus heading weight), as static tokens the S1.07 stylesheet generator
  will read. Deferred-from-S1.02 setter landed: `set_site_theme` on the
  account door, storing the canonical serialization; schema violations map
  to named `Conflict`s like the sections gate. `site_model`'s id-token rule
  is now shared (`pub(crate) valid_id_token`) so "a valid id" means one
  thing across the sites schema family.
- **Verified:** `cargo fmt`; `SQLX_OFFLINE=true cargo clippy -p alo-store
  --all-targets` zero warnings; full `cargo test -p alo-store` green on the
  local docker Postgres (90 unit incl. 8 new theme tests — ≥6-presets +
  unique-wellformed-ids, hex format + **WCAG AA contrast ≥4.5:1 enforced on
  every text pairing of every shipped palette**, full/minimal round-trips,
  version-error precedence, unknown prop/preset/blob-ref rejection,
  from_stored pristine + defensive paths; isolation suite 22 with the sites
  test extended: outsider `set_site_theme` cleanly denied, co-tenant write
  lands canonically and reads back, four off-schema payloads rejected
  without touching the stored value). Manual pass: psql shows real rows
  with the pristine `{}` default; the terra write was read back from the
  real DB through the store inside the isolation test.
- **Cuts/flags:**
  - v1 is presets-only, no free-form colors — rejected alternative recorded
    in the module doc: arbitrary user hex would break the build-time
    contrast guarantee the preset test enforces.
  - Logo/favicon blob refs are shape-checked only (same posture as S1.03's
    `SiteImage.blob_id`); ownership resolves through the tenant-scoped blob
    door at render/serve time.
  - Preset display names ("North", "Terra", …) are product proper nouns,
    deliberately not i18n'd — documented on `ThemePreset`.
  - `Site.theme` stays an opaque `Value` on read; renderers use
    `SiteTheme::from_stored`. CHANGELOG untouched (no user-visible surface
    until S1.10/S1.14).
- **Next:** S1.06 (renderer crate `products/sites/alo-sites`, page JSON +
  theme → full HTML document, golden tests).

## S1.06 — renderer crate `products/sites/alo-sites` (2026-08-07)

- **Shipped:** new workspace crate `alo-sites` (library-first; the axum
  service is S1.09) with the pure `render` module: page JSON + theme → one
  complete HTML document. `render_page(SiteRenderContext, PageRenderContext)`
  emits head (charset/viewport, title `seo_title` or `<page> — <site>`, meta
  description, canonical, OG type/site_name/title/description/url, og:image
  from the first hero image, favicon from theme, one stylesheet link) and a
  landmarked body: skip link → `nav` sections as `<header>` → one `<main>` →
  `footer` sections as `<footer>` (rule documented: a mid-page nav still
  lands in the header region — valid landmarks outrank literal order). One
  fragment builder per section type in `render/sections.rs` (h1 only in hero,
  h2 per section, h3 per item, `<details>` FAQ, `alt` on every `<img>`,
  stable `s-<kind>` class hooks for the S1.07 stylesheet). Read-side
  tolerance per the design note: `sections_lenient` parses per-entry and
  skips unknown/newer sections with a `tracing` warning — never a 500.
  Defense in depth independent of the write gate: every string through
  `esc()`, every link target re-checked against the href allowlist (unsafe →
  inert `#`, in `render/html.rs`). Visitor-facing chrome strings (skip link,
  menu, form labels) externalized in `render/strings.rs` (`UiStrings`, `EN`).
  Contact form: posts `/f/<form_id>`, fixed v1 field contract name/email/
  message + visually-hidden `website` honeypot (aria-hidden, tabindex −1),
  `data-success` attribute; without a `form_id` the section renders text
  only. Public-path contract documented in `lib.rs`: `/assets/site.css`,
  `/assets/img/<blob_id>`, `/f/<form_id>` — changing these means
  re-rendering every snapshot.
- **Verified:** `cargo fmt --check`; `SQLX_OFFLINE=true cargo clippy -p
  alo-sites --all-targets` zero warnings; `cargo test -p alo-sites` green —
  13 golden files (one per section type + a themed full-page golden with
  logo/favicon/SEO, blessed via `UPDATE_GOLDENS=1` then re-run clean) and 11
  behavior tests (head/OG/canonical exact strings, landmark order, lenient
  skip incl. newer schema_version, script + attribute-injection escaping,
  javascript: href rendered inert, honeypot + fixed fields, alt on every img
  across the full corpus, theme logo/favicon paths). Manual pass: read the
  full-page golden byte-for-byte — structure, escaping (`&#39;`, `&amp;`),
  and all head tags check out. No storage/routes touched → wrong-tenant and
  wire-verify gates don't apply (pure library).
- **Cuts/flags:**
  - Feature icons: the schema's icon token renders nothing yet (we ship no
    icon set); the fallback path is the only path until an icon set arrives
    with the stylesheet slice, or the prop is retired at wave review.
  - Byte-budget tests (CSS < 50KB, HTML < 100KB) are S1.07's, with the
    stylesheet; the nav toggle button is inert markup until S1.07's JS.
  - Site-level locale selection doesn't exist yet — `EN` chrome strings
    only; fr/nl land at the wave review (S1.31).
  - CHANGELOG untouched: rendering library only, no served surface yet.
- **Next:** S1.07 (stylesheet generation from theme tokens + byte budgets).

## S1.07 — stylesheet from theme tokens + the behavior script (2026-08-07)

- **Shipped:** new pure module `alo_sites::stylesheet` — `stylesheet(&SiteTheme)
  -> String`, the complete CSS document served at `/assets/site.css`: a
  generated `:root` custom-property block from the resolved preset's
  palette/typography tokens over one static mobile-first sheet (single 48rem
  breakpoint, contained images, section layouts for all twelve `s-<kind>`
  hooks, honeypot visually hidden, skip-link focus reveal). Color use sticks
  to store-proven WCAG pairings; the two pairings the sheet adds (links/
  secondary buttons: `primary` on `background`/`surface`) are now pinned in
  the store's contrast test — all presets clear ≥ 4.5:1. Plus the page's
  **entire JS budget**: a static `render/script.rs` block (menu toggle via
  `aria-expanded` + fetch form-submit that swaps in the `data-success`
  message, native-submit fallback on any failure), **inlined** — rejected
  alternative recorded in the module doc: a fourth `/assets/site.js` path
  would widen the public-path contract for a script with zero user data.
  Appended only when the page has a nav or a live form; both behaviors are
  progressive enhancement (no-JS menu renders expanded — collapse only
  exists under the script's `js` class; forms post natively). Forms now
  always carry `data-success` (custom or the new externalized
  `form_success` default string).
- **Verified:** `cargo fmt --check`; `SQLX_OFFLINE=true cargo clippy -p
  alo-sites -p alo-store --all-targets` zero warnings; `cargo test -p
  alo-sites` green (21 tests: new stylesheet_rules suite — site.css golden
  for the default preset, per-preset token wiring + brace balance +
  **CSS < 50KB budget**, self-containment (no `@import`/`@font-face`/`url(`/
  absolute URL — the zero-external-requests promise, mechanically), contract
  selectors incl. the toggle's `[aria-expanded="true"] + ul` pair, pristine-
  theme fallback; render_rules gains script-inclusion exactness + default
  data-success; full-page golden now asserts **HTML < 100KB** — actual:
  CSS 7.6KB, page 5.9KB). Full `cargo test -p alo-store` green on local
  docker Postgres (200 unit + isolation suites) with the extended contrast
  test. Manual pass: read the re-blessed golden diffs byte-for-byte — the
  only change is the script block between `</footer>` and `</body>`, and
  site.css's `:root` carries the north tokens exactly. No storage/routes
  touched → wrong-tenant and wire-verify gates don't apply (pure library).
- **Cuts/flags:**
  - Feature-icon rendering still absent (S1.06 flag stands) — the sheet
    styles no icon slot; retire-or-ship decision at wave review (S1.31).
  - The nav collapse honors only screen width, not menu length; fancy
    behaviors (sticky nav, scroll effects) are out — the JS budget is the
    two behaviors, by design.
  - CHANGELOG untouched: still a rendering library, nothing served yet
    (first user-visible surface lands with S1.09/S1.10).
- **Next:** S1.08 (publish flow: immutable per-page published snapshots +
  site publish state).

## S1.08 — publish flow: immutable snapshots + publish state (2026-08-07)

- **Shipped:** migration `0057_site_publishes.sql` — `site_publishes` (theme
  frozen at publish time, published_by/at, cascading tenants → sites →
  publishes) + `site_page_snapshots` (slug/title/sections/SEO/nav/home frozen
  per page; **deliberately no FK to `site_pages`** — a snapshot must survive
  the draft page being edited or deleted, that's the immutability property) +
  the published-set pointer `sites.published_publish_id` (composite FK to
  site_publishes so it can only name a same-tenant publish; no referential
  action — publishes die only by the site cascade). New `SitePublishId`;
  `platform/alo-store/src/site_publish.rs` on the account door:
  `publish_site` (one transaction: site row locked FOR UPDATE so concurrent
  publishes serialize → named Conflicts for zero pages / no home page →
  publish + snapshot rows copied INSERT…SELECT inside SQL so the snapshot is
  byte-what the write gates admitted → pointer flip + status `live`),
  `unpublish_site` (pointer NULL + status `draft`; history retained;
  idempotent), `current_site_publish`, `site_publish_snapshots` (scoped
  through the site; wrong tenant or wrong site reads as empty/None,
  indistinguishable from absent).
- **Verified:** `cargo fmt --check`; `SQLX_OFFLINE=true cargo clippy -p
  alo-store --all-targets` zero warnings; full `cargo test -p alo-store`
  green on the local docker Postgres (200 unit + all suites; isolation 23
  with the new `site_publishes_freeze_immutable_snapshots_and_scope_by_tenant`:
  empty/homeless site refused, publish freezes pages+theme, then edit +
  retitle + add + **delete** + retheme and the published set doesn't move a
  byte — republish makes a NEW set while the old survives; outsider tenant
  cleanly denied on publish/unpublish/reads; same-tenant cross-site
  addressing reads empty; unpublish keeps history; site delete cascades
  publishes+snapshots through the pointer FK without error). Manual pass:
  `\d site_publishes` / `\d site_page_snapshots` in psql show PKs, indexes,
  cascade FKs and the pointer FK exactly as designed; real snapshot rows
  from the test run read back with frozen slug/home/nav values.
- **Cuts/flags:**
  - No publish-history list API — nothing consumes it yet; the index
    (`site_publishes_by_site`, newest first) and immutable rows are the S2
    rollback substrate, the accessor lands with a consumer.
  - Snapshot retention is unbounded by design (immutable history); revisit
    with quotas if it ever binds.
  - `unpublish_site` shipped (small, completes the state machine) though the
    queue text named only publish; publish UI copy is S1.15's.
  - CHANGELOG untouched: store flow only — the first user-visible surface
    lands with S1.09/S1.10.
- **Next:** S1.09 (`alo-sites` public service: Host resolution → published
  snapshots, cache, /healthz, Host-isolation tests).

## S1.09 — alo-sites public service: Host → published snapshots (2026-08-07)

- **Shipped:** the anonymous serving half of the two-service boundary.
  Store side: `platform/alo-store/src/site_public.rs` — `SitePublicStore`,
  a **separate read-only door** on a plain pool (deliberately not `Store`:
  no system ops, no blob backend, no way to open a tenant/account door);
  `resolve_published(subdomain)` is the one indexed read (sites ⋈
  site_publishes on the published-set pointer, backed by
  `sites_subdomain_unique`) → `PublishedSite` whose **tenant field is
  private**, and `published_pages(&PublishedSite)` scopes by that resolved
  pair — serving rows the Host lookup didn't lead to is unrepresentable.
  Service side: `products/sites/alo-sites/src/serve.rs` (+ `serve/{config,
  host,cache,rendered}.rs`) and the `alo-sites` binary (`src/main.rs`, runs
  **no migrations** — alo-jmap owns the schema). Host parsing reuses
  `validate_subdomain` (ports/FQDN-dot/case tolerated; apex, nested labels,
  IP literals, lookalike suffixes all fall through). Cache is publish-keyed:
  the resolver read runs per request (republish/unpublish visible on the
  next request, ever-stale impossible by construction), rendering happens
  once per publish (bounded map, 512 sites, arbitrary eviction). Response
  contract: strong `ETag "<publish>:<path>"` + `If-None-Match` → 304,
  `Cache-Control: public, max-age=60`, nosniff, trailing-slash tolerance;
  unknown/unpublished host → one byte-identical generic 404 (no existence
  leak); unknown path on a live site → a **themed** 404 (`render_not_found`
  in the render lib + 3 new `UiStrings` entries, en; fr/nl at S1.31);
  DB trouble → terse 503 + Retry-After, internals never on the wire;
  non-GET/HEAD → 405 + Allow. Env contract: `DATABASE_URL`, `SITES_DOMAIN`,
  `ALO_SITES_ADDR` (default 0.0.0.0:8081).
- **Verified:** `cargo fmt --check`; `SQLX_OFFLINE=true cargo clippy -p
  alo-sites -p alo-store --all-targets` zero warnings; `cargo test -p
  alo-sites` green (host-parsing unit tests + 4 in-process integration
  tests via tower::oneshot against the real router + compose Postgres:
  response contract incl. 304/405/css, **Host isolation** — A's host never
  serves B's markers/theme/pages, unknown ≡ unpublished byte-identical,
  republish flips on the next request while draft edits never leak);
  full `cargo test -p alo-store` green (207 unit + all suites; isolation
  24 with the new `public_resolver_scopes_by_subdomain_and_never_leaks_drafts`).
  Manual wire pass with the real binary on 127.0.0.1:8081 against docker
  `alo-pg`, real curl: healthz 200 `ok`; live host → 200 text/html,
  `cache-control: public, max-age=60`, `etag: "i48oI0wvzfP2L4JR0kLkaQ:/"`,
  nosniff, canonical `https://<sub>.sites.test/`; If-None-Match → 304
  size=0; `/assets/site.css` → text/css with the north preset tokens;
  iso-a host serves ALPHA-ONLY only, iso-b host BETA-ONLY only; unknown
  host → 404 `<title>Page not found</title>` (generic); live host unknown
  path → 404 `<title>Page not found — Alpha Site</title>` (themed);
  POST → 405; apex host → 404.
- **Cuts/flags:**
  - `/assets/img/<blob_id>` (the third public path) is NOT served yet —
    no published fixture carries images until logo/gallery upload lands;
    wiring it lands with S1.14 (Drive/blob refs). The renderer already
    emits those URLs; until S1.14 an image-bearing page would 404 its
    images. `/f/:form_id` is S1.16 by design.
  - No CSP header on served pages (inline behavior script + inline 404
    style make it low-value churn now); revisit at wave review.
  - Eviction is arbitrary-at-bound, not LRU — deliberate until real
    traffic exists; noted in `serve/cache.rs`.
  - 503 path exercised by code review only (would need killing Postgres
    mid-test; the mapping is 6 lines, all errors → one static body).
- **Human inbox (deploy, when the wave ships):** production needs the
  `alo-sites` container in compose (env above), the `SITES_DOMAIN`
  purchase, wildcard DNS + wildcard/on-demand TLS at Caddy routing
  `*.<SITES_DOMAIN>` → alo-sites. Deliberately not touched by the loop.
- **Next:** S1.10 (edit API in alo-jmap: `/sites/*` CRUD + section ops +
  publish, Problem errors, wire transcript).

## S1.10 — edit API in alo-jmap: `/sites/*` (2026-08-07)

- **Shipped:** `products/mail/alo-jmap/src/sites.rs` + registration in
  `server.rs`/`lib.rs` (additive lines) — the authenticated edit half of the
  two-service boundary. Sites: `GET/POST /sites`, `GET /sites/subdomain-check`
  (live taken/free for the create form), `GET/PUT/DELETE /sites/{id}` (PUT
  takes `{name?, subdomain?}`, empty PUT is a named 422), `PUT
  /sites/{id}/theme` (body = the theme envelope, through the store's theme
  gate), `POST /sites/{id}/publish` → `{publishId, status:"live"}`, `POST
  /sites/{id}/unpublish` (idempotent). Pages: `GET/POST /sites/{id}/pages`
  (list stays lean — no sections), `PUT /sites/{id}/pages/order` (full
  permutation), `GET/PUT/DELETE /sites/{id}/pages/{pid}` (PUT does partial
  title/slug/seoTitle/seoDescription; SEO merges over the two-field store
  setter — absent keeps, blank clears), `POST .../home`. Sections, addressed
  **by index** into the ordered envelope (no ids by design — the S1.27 AI ops
  speak the same vocabulary): `PUT .../sections` (atomic full set), `POST
  .../sections` `{section, index?}`, `PUT/DELETE .../sections/{index}`,
  `POST .../sections/{index}/move` `{to}` — read-modify-write through the
  schema write gate; every op answers the canonical stored envelope. Error
  contract per the design note: 401 unauthenticated (WWW-Authenticate:
  Bearer); anything not resolving in the caller's tenant → 404; **every**
  rule violation → 422 with the store's rule-naming message (the sites store
  spells them all as `Conflict`, so this module's map sends `Conflict` →
  422, not 409 — documented in the module doc); malformed JSON → 400 notJSON.
- **Verified:** `cargo fmt`; `SQLX_OFFLINE=true cargo clippy -p alo-jmap
  --all-targets` zero warnings; **full `cargo test -p alo-jmap` green** on
  the local docker Postgres, including the new `tests/sites_http.rs` (7
  tests through the real router + real DB: 401 across the route families,
  site lifecycle incl. delete-releases-subdomain, page lifecycle incl. SEO
  partial-merge/blank-clears and home promotion, section add/update/move/
  remove + full-set + out-of-range and malformed index, theme gate + publish
  preconditions + live/unpublish flow, and the mandatory wrong-tenant
  barrage — 18 verbs against tenant B's ids all answer A with 404 and leak
  nothing, B's data untouched after). Manual wire pass against the real
  debug binary on 127.0.0.1:8080 + docker `alo-pg`, real curl: no token →
  401 `{"detail":"missing or invalid bearer token"}` + `www-authenticate:
  Bearer`; create happy → full site JSON (draft, `{}` theme); `UPPER` → 422
  "subdomain may only contain lowercase letters, digits, and hyphens";
  `mail` → 422 "subdomain is reserved"; re-claim → 422 "subdomain is already
  taken"; publish-no-pages → 422 "site has no pages to publish"; slug
  `blog` → 422 "slug is reserved"; add hero/cta → canonical envelopes;
  `carousel` → 422 naming the 12 known types; `javascript:` href → 422
  naming the href rule; move/remove reshuffle correctly; index 5 → 422 "no
  section at index 5 (the page has 2)"; bad preset → 422; terra lands;
  publish → `{"publishId":"S9iFT9_n69lckiOdzNTrFw","status":"live"}`; GET
  site shows `publish{id, publishedAt}` + status live; unpublish → draft;
  `{not json` → 400 notJSON. psql after: the sites row (terra, pointer
  cleared after unpublish), 2 page rows (home flag, nav order, 1 section
  stored canonically), publish row with 2 frozen snapshots.
- **Cuts/flags:**
  - No optimistic concurrency on section read-modify-write (single-editor
    assumption; an If-Match seam is S2) — documented in the module doc.
  - No draft-preview endpoint (S1.13 by queue design), no theme-preset
    listing route (lands with the S1.14 theme UI), no publish-history list
    (S2 rollback substrate stays store-only).
  - `GET /sites/{id}` additionally returns the current publish
    (`publish: null | {id, publishedAt}`) — small addition beyond the queue
    text, the S1.15 status chip needs it.
  - **Caddyfile note:** `/sites` is a NEW top-level route prefix — production
    Caddy needs it routed to alo-jmap at the next deploy (same as `/billing`).
  - `cargo fmt` wanted to reflow 7 pre-existing alo-jmap modules this item
    never touched (agent/base/drive/spaces/tasks/wopi/workspace_search —
    import-order + wrapping, likely a rustfmt style-edition delta with the
    other machine). Reverted deliberately: formatting churn on the business
    track's active files invites rebase conflicts; my own files are
    fmt-clean. Flagged for a human to align rustfmt versions at wave review.
  - CHANGELOG: user-voice entry added (first user-visible sites surface).
- **Next:** S1.11 (web module skeleton: rail entry, site list + create,
  page list; i18n en).

## S1.11 — web module skeleton `web/src/sites` (2026-08-07)

- **Shipped:** the Sites web module, cut from the Tasks/Billing module cloth.
  Rail entry **"Websites"** (Globe) registered in the workplace product
  surface only (ADR 0036 — the suite sells it, the standalone mail app never
  sees it). `web/src/sites/`: `api.ts` — its own thin REST client over the
  wire-verified `/sites/*` surface (S1.11 slice only: list/create site,
  get site, subdomain check, list/create page; methods land with their
  consumers), `SitesError` carrying the server's rule-naming detail,
  `sitesMessage` fallback helper; `types.ts` — wire types trimmed to what the
  screens render; `SitesListView` — site table (name → opens the site,
  address mono, live/draft chip), empty state, load-failure banner;
  `NewSiteDialog` — name + address with a **live taken/free check**
  (350 ms debounce, sequence-guarded against out-of-order answers,
  advisory only: submit always allowed, the server's 422 shown verbatim);
  `SiteView` — site header (back link, name, address, status chip) + page
  list (title, home badge, /path) at `/sites/:siteId`, stale/foreign id
  reads as not-found with the way back; `NewPageDialog` — title/path/home,
  the home flag **defaulting on for the first page**; `parts.tsx` — the
  module's own dialog chrome/empty/error/field pieces (deliberately NOT
  imported from billing: cross-track coupling; promoting this chrome into
  `ds` once three modules carry it is a wave-review candidate); module CSS;
  ~40 new `i18n/en.ts` strings (additive block at the catalog's end).
- **Verified:** `npx tsc --noEmit` clean; `npx eslint` on all changed files
  zero warnings; **full `npm test` green — 178 tests / 25 files**, incl. 10
  new module tests driving the REAL client + views over a recording fake
  fetch (list renders the API's answer with status chips; empty state;
  load-failure banner; live check hits `/sites/subdomain-check` and shows
  free/taken/the server's 422 rule sentence; create POSTs exactly
  `{name, subdomain}` and navigates into the new site; a 422 refusal shows
  in the dialog which stays open; page list with home badge; 404 → not-found
  + back link; create-page POSTs `{title, slug, home}` with home defaulted
  on the first page and the list reloads); `npm run build` clean. No new
  HTTP routes and no storage touched → the wire/wrong-tenant gates are the
  server's (proven in S1.10); the component tests pin the exact request
  shapes this client sends to that verified surface.
- **Cuts/flags:**
  - The page-create dialog is a small deliberate addition beyond the queue's
    "page list": no other item owns page-create UI (S1.12 owns sections),
    and a page list with no way to create a page is dead UI. Recorded here.
  - No site rename/delete/publish UI (S1.15), no section editor (S1.12), no
    theme UI (S1.14), no list search (lists are short), fr/nl at S1.31.
  - The create form shows the bare label only; the "goes live at
    `<sub>.<domain>`" copy is S1.15's, when the web learns the sites domain.
  - Rejected alternative (recorded in `parts.tsx`): importing billing's
    dialog chrome — same look, but couples the two tracks' modules; own
    copies now, promotion to `ds` at wave review.
  - `npm ci` had to be run on this machine (fresh checkout, no
    node_modules) — environment note only.
- **Next:** S1.12 (web editor core: section stack + per-type prop forms +
  save).

## S1.12 — web editor core: section stack + per-type prop forms (2026-08-07)

- **Shipped:** the visual page editor at `/sites/:siteId/pages/:pageId`
  (page titles in the site view now link into it). `PageEditorView` — the
  section stack as cards (type name + a summary line: the section's own
  heading, or a count of its entries), native drag-reorder with arrow-button
  fallback, edit, and two-click-confirm delete; `SectionPicker` — the add
  dialog: twelve tiles, each with an inline-SVG schematic thumbnail, the
  type's name and a one-liner; `SectionForm` — the per-type prop forms over
  shared primitives (text/long-text/link/image fields and a generic
  repeating-entries editor for list props; pricing bullets edit as
  one-per-line text). **Design:** every gesture is one call to the
  wire-verified S1.10 section ops (add is form-first → POST, edit → PUT
  index, reorder → move, delete → DELETE) and the stack always renders the
  canonical envelope the server answers — no local dirty buffer to lose, and
  a 422 points at the exact gesture that broke the rule (rejected
  alternative, recorded in the view doc: a dirty buffer with one atomic
  save — fewer requests, but lost-work risk and ambiguous refusals). The
  client re-states NO validation: refusal sentences are the store's,
  verbatim, and the forms drop untouched-blank optionals/rows to absent keys
  on save. Supporting files: `sections.ts` (the TS mirror of schema v1),
  `sectionDrafts.ts` (the editable spelling + toDraft/toSection — props the
  forms don't offer, a feature's `icon` and a contact form's `form_id`, ride
  through edits untouched), `sectionInfo.ts` (labels/descriptions/
  summaries); ~90 new `i18n/en.ts` strings (additive block).
- **Verified:** `npx tsc --noEmit` clean; `npx eslint` on all changed files
  zero warnings; **full `npm test` green — 187 tests / 26 files**, incl. 9
  new editor tests over the recording fake fetch driving the REAL client and
  views: stack renders stored sections in order; picker offers all twelve;
  add POSTs exactly the typed section (trimmed, `toEqual`-pinned so blank
  optionals are ABSENT); a list section sends every added entry; edit opens
  prefilled and PUTs to the index with the untouched subheading riding
  along; `form_id` survives an edit; move-down POSTs `{to:1}`; delete needs
  the second click and one click alone writes nothing; a 422 shows the
  server's sentence in the open dialog; a stale page id reads as the error
  with the way back. `npm run build` clean. No new routes and no storage
  touched → the wire/wrong-tenant gates are the server's (proven in S1.10);
  the tests pin the exact request shapes sent to that verified surface.
- **Cuts/flags:**
  - Image props are blob-id + alt-text inputs until the S1.14 Drive picker
    (the field's hint says so); a pasted Drive file id works today.
  - `contact_form.form_id` (S1.16) and the feature `icon` token (renderer
    ships no icons — S1.06 flag) are preserved-not-offered by the forms.
  - Repeating entries have add/remove but no inner reorder — the queue's
    reorder ask is the section stack; revisit if users hit it.
  - Drag-reorder is native HTML5 DnD, which jsdom cannot drive — the tests
    exercise the arrow buttons, which share the same `move()` path.
  - No optimistic concurrency on the read-modify-write ops (S1.10's
    single-editor flag stands; If-Match is the recorded S2 seam).
  - The repo's `exactOptionalPropertyTypes` means optional wire props are
    spelled `?: X | undefined` so computed blanks can be built and then
    dropped by `JSON.stringify` — noted in `sections.ts`.
- **Next:** S1.13 (live preview: authenticated draft-render endpoint in
  alo-jmap + iframe preview pane).

## S1.13 — live preview: draft-render endpoint + preview pane (2026-08-07)

- **Shipped:** the editor now shows the page while it is built. Render lib:
  one private `render_document` behind both spellings — `render_page`
  (public serving, `<link>` to `/assets/site.css`) and the new
  `render_page_preview` (the same document with the generated stylesheet
  inlined in a `<style>` block, because the public asset paths do not
  resolve on the edit origin); the stylesheet self-containment test now also
  forbids `</` so embedding the sheet verbatim can never close the block.
  Edit API: `GET /sites/{id}/pages/{pid}/preview` — authenticated like every
  edit route, renders the DRAFT with `SiteTheme::from_stored` + the site's
  future public origin (`https://<sub>.<SITES_DOMAIN>`, env with the
  alosites.com default) in canonical/OG, answers `text/html` with
  `Cache-Control: no-store` (a draft has no cache life). Web: the editor is
  now a two-pane layout — section stack left, preview right (sticky,
  stacking on narrow screens); the pane holds a sandboxed iframe
  (`sandbox="allow-scripts"`, document via `srcdoc` — it may run its own
  menu script but never touches the app origin), refetches whenever a save
  lands (keyed on the envelope the last op answered, so a refused gesture
  does not refresh), and a desktop/phone toggle lays the document out at
  375px. `SitesApi.pagePreview` answers text; the non-2xx→`SitesError` map
  is now shared (`#rejectFailed`) between the JSON and text paths. 5 new
  i18n en strings.
- **Verified:** `cargo fmt`; `SQLX_OFFLINE=true cargo clippy -p alo-sites
  -p alo-jmap --all-targets` zero warnings; `cargo test -p alo-sites` green
  (28 — incl. the new no-drift pin: for EVERY shipped preset, preview ==
  published document with exactly the stylesheet reference swapped,
  byte-for-byte) and full `cargo test -p alo-jmap` green (30 suites; sites_http
  now 8 — the new preview test: self-contained document, no-store, follows an
  edit on the next fetch; the 401 barrage and the wrong-tenant barrage both
  extended with the preview route). Web: `npx tsc --noEmit`, eslint clean,
  **full `npm test` green — 202 tests / 27 files** incl. 4 new preview-pane
  tests (server document reaches the sandboxed frame's srcdoc; a successful
  save refetches while a refused op provably does not; width toggle flips
  aria-pressed; a failed preview shows its own error and never blocks
  editing); `npm run build` clean. Manual wire pass against the rebuilt
  debug binary + docker `alo-pg`, real curl: hero with `cta` → 422 naming
  the real props (gate live on the wire), `primary_cta` → 200; preview →
  200 `text/html; charset=utf-8`, `cache-control: no-store`, `<style>`
  inlined, zero `/assets/site.css`, `Bread &amp; butter` escaped, canonical
  `https://wire-preview-s113.alosites.com/`; no token → 401; bogus page id →
  404; PUT the hero then re-fetch → the new heading, the old one gone;
  fixture site deleted after.
- **Cuts/flags:**
  - This iteration ADOPTED uncommitted S1.13 work found in the tree (the
    previous invocation died before finishing): the Rust endpoint, render
    split, and preview pane were in place but unfinished — the 5
    `sitesPreview*` i18n strings, all web tests, CHANGELOG, and
    QUEUE/STATE were missing. Everything was read line-by-line, finished,
    and re-verified through the full gates before committing as one item.
  - The preview refetches the whole document per save (no diffing/morphing)
    — right-sized until pages get heavy; revisit only on real complaints.
  - `sites_domain()` reads env once per process (OnceLock); a changed
    `SITES_DOMAIN` needs a restart — same posture as alo-sites' own config.
  - The rustfmt-version delta from S1.10 struck again (fmt wanted to reflow
    7 untouched business-track modules incl. `agent.rs`, active on the other
    machine); reverted the churn again — the S1.10 flag for aligning rustfmt
    versions at wave review stands.
  - Environment note: the first `cargo test -p alo-jmap` build OOM-killed
    parallel rustc (missing-rlib errors); `-j 2` builds clean — same as the
    S1.03 note.
- **Next:** S1.14 (theme UI: preset picker + logo/favicon upload via Drive;
  preview updates).

## S1.14 — theme UI + the whole image path (2026-08-07)

- **Shipped:** the theme becomes a thing users touch, and images become real
  end-to-end. **Web:** `ThemeDialog` (preset cards rendered from the server's
  tokens — swatches + the preset's own heading font — plus logo/favicon
  upload/replace/remove rows), opened from BOTH the site view and the page
  editor; applying PUTs the full envelope through the wire-verified theme
  gate and the editor preview refetches (new `previewEpoch` — the preview
  depends on the theme, not only the sections). Uploads go through Drive:
  new additive `JmapClient.driveUploadBlob` (one upload, registered as a
  drive file, returns the blob id) — rejected alternative: the bare
  `uploadFile`, whose contract says unreferenced blobs may be GC'd; a
  logo must outlive any future GC and stay user-visible. Section image
  fields got the picker S1.12's hint promised: an Upload button beside the
  id input (same Drive path). **Edit API:** `GET /sites/theme-presets`
  (authed; ids/names/palette/typography, camelCase) for the picker;
  the draft preview now inlines theme + section images as `data:` URIs
  (public paths don't resolve on the edit origin; > 4 MiB or non-image →
  public-path fallback). **Render lib:** `SiteRenderContext.images:
  ImageSources` (`PublicPaths` | `Inline(map)` with per-id fallback;
  `og:image` deliberately always the absolute public URL) — rejected
  alternative: post-processing the rendered HTML string. **Public service
  (closes the S1.09 flag):** `/assets/img/<blob_id>` now serves — membership
  gate first (`RenderedSite.serves_image`: the set collected from the
  publish's frozen theme + lenient sections, so servable ≡ shown), then the
  tenant-scoped read `SitePublicStore::published_image` (blob row by the
  resolved site's private tenant, bytes from the new blob backend handle);
  image content types only (allowlist in the new
  `alo-store/src/site_assets.rs`, shared with the preview), `ETag
  "img:<id>"` + 304, `public, max-age=3600`, nosniff, and
  `Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'`
  so an SVG opened as a document can't run script on the site origin.
  `alo-sites` env gains **required `ALO_BLOB_DIR`**. Store:
  `AccountStore::site_image` (tenant-scoped, image-typed blob read for the
  preview) and `Section::image_blob_ids()` (exhaustive match — a new
  variant fails to compile until it declares its images).
- **Verified:** fmt; `SQLX_OFFLINE=true cargo clippy -p alo-store -p
  alo-sites -p alo-jmap --all-targets` zero warnings; **full test suites
  green on docker Postgres**: alo-store 318 unit + all suites (isolation
  gains `site_images_scope_by_tenant_and_refuse_non_images`: own image
  serves, own non-image reads as absent, **outsider tenant reading the blob
  id gets clean None**, and `published_image` through B's resolved site
  never reaches A's bytes), alo-sites 30 (serve_http gains
  `serves_exactly_the_published_images`: unreferenced same-tenant blob /
  foreign blob / referenced-but-HTML blob / garbage id all themed-404,
  referenced logo serves with the full header contract + 304; render_rules
  gains the Inline-sources pin incl. og:image exemption; the preset no-drift
  pin still holds byte-for-byte), alo-jmap 284 unit + all suites (sites_http
  now 10: presets shape + 401, preview inlines the logo as base64 while the
  non-image falls back to the public path). Web: tsc, eslint, `npm run
  build` clean; **208 tests / 28 files** incl. 6 new (presets render +
  exact envelope PUT with absent keys, upload feeds blob id in, stored logo
  prefills + remove drops the key, 422 sentence stays in the open dialog,
  theme apply refetches the preview, hero image upload fills the id the
  section then saves). Manual wire pass, real curl against both debug
  binaries + docker `alo-pg`: presets 401 then 200 (7 presets, north
  first); real 70-byte PNG through `/jmap/upload` + `/drive/files`;
  `vaporwave` → 422 naming the preset rule, `not/a/token` logo → 422,
  terra+logo → ok; preview `no-store` with `data:image/png;base64,iVBOR…`
  inline, zero `/assets/img/` for the logo, terra tokens in the inlined
  style; publish → the served home references `/assets/img/<id>`, the image
  answers 200 `image/png` + etag + CSP with **bytes identical to the
  uploaded PNG** (cmp), If-None-Match → 304 size 0, garbage id → themed 404
  (`Page not found — Wire Theme Co`), apex host → 404; fixture site deleted,
  host off the air.
- **Cuts/flags:**
  - Responsive image derivatives/resizing stay S2 (design-note out-of-scope);
    the service serves original bytes, one blob read per request (no image
    byte-cache — revisit with real traffic, noted in `serve.rs`).
  - Theme v1 stays presets-only; free-form colors remain rejected (contrast
    guarantee).
  - The dialog shows uploaded-state text, not a thumbnail — the preview pane
    IS the thumbnail (it renders the real logo); revisit only on complaints.
  - `SitePublicStore` constructors now take the blob backend (breaking only
    in-repo callers; no public wire contract touched). The public door doc
    was updated: it now holds a blob handle, reachable only through
    `published_image`.
  - **Deploy note (human inbox):** the production `alo-sites` container will
    need `ALO_BLOB_DIR` mounted to the same blob directory `alo-jmap` writes
    (read-only is fine), alongside the S1.09 compose/Caddy/DNS items.
  - Blob GC does not exist today; when one lands it MUST treat site
    references (theme logo/favicon, section images, later post covers) as
    live references — flagged here so the future implementer inherits the
    knowledge.
  - The rustfmt-version delta struck again (fmt reflowed 7 untouched
    business-track modules); reverted the churn again — the S1.10 flag about
    aligning rustfmt versions at wave review stands.
- **Next:** S1.15 (publish UI: publish button + "goes live at" copy +
  status chips; STATE human-inbox note for production serving).

## S1.15 — publish UI + the web learns the sites domain (2026-08-07)

- **Shipped:** publishing becomes a button. **Edit API:** one new route,
  `GET /sites/config` → `{"domain": <SITES_DOMAIN>}` — the deployment-wide
  apex the web composes "goes live at" copy and live links from, instead of
  hardcoding a domain (the missing piece S1.11 recorded; publish/unpublish
  routes existed since S1.10). Authenticated like every `/sites/*` route.
  Rejected alternative (recorded in the module doc + this entry): a per-site
  `url` field in site JSON — duplicates a derivable composition and leaves
  the create form (no site yet) without a domain source; one config route
  serves both consumers. **Web:** the site view gains a publish bar — draft:
  **Publish** + "Publishing puts this site live at `<sub>.<domain>`."; live:
  "Your site is live at" + the address as a real `https://` link
  (new-tab, noreferrer) + **Publish changes** + **Take offline** with the
  module's two-click-confirm pattern (first click arms and turns the button
  red, second acts). A refused publish shows the store's 422 sentence
  verbatim inline (`role="alert"`), never swallowed; a failed config fetch
  degrades — the address copy stays off, publishing still works. The
  new-site dialog now previews "Your site will live at `<sub>.<domain>`."
  under the live check (the copy S1.11 deferred here). Live/draft chips were
  already shipped in S1.11 and are unchanged; they now flip through the
  publish flow on this screen. `SitesApi` gains `config` / `publishSite` /
  `unpublishSite`; 9 new `i18n/en.ts` strings (additive block).
- **Verified:** `cargo fmt`; `SQLX_OFFLINE=true cargo clippy -p alo-jmap
  --all-targets` zero warnings; **full `cargo test -p alo-jmap` green** on
  docker Postgres (298 unit + all suites; sites_http now 11 — the 401
  barrage extended with `/sites/config`, plus `config_names_the_sites_domain`).
  Web: `npx tsc --noEmit` clean, eslint zero warnings on all changed files,
  **full `npm test` green — 231 tests / 29 files** incl. 4 new (draft shows
  the composed goes-live copy and Publish POSTs `/publish` then renders the
  live chip + the address as a link with the exact href; a 422 refusal shows
  the server's sentence and stays draft; Take offline writes nothing on the
  first click and POSTs `/unpublish` on the confirm click; the create form
  previews the lowercased typed label against the fetched domain);
  `npm run build` clean. Manual wire pass, real curl against the rebuilt
  debug binary + docker `alo-pg`: `/sites/config` no token → 401 +
  `www-authenticate: Bearer`; with token → 200 `{"domain":"alosites.com"}`;
  fresh site → publish-no-pages → 422 "site has no pages to publish" (the
  sentence the UI shows verbatim); home page + publish →
  `{"publishId":…,"status":"live"}`; GET site → status live +
  `publish{id, publishedAt}`; unpublish → draft + `publish: null`; fixture
  site deleted after.
- **Cuts/flags:**
  - No publish-history UI and no published-at display — the chip + link are
    the S1.15 ask; history stays the S2 rollback substrate.
  - The sites list keeps showing the bare subdomain (no config fetch there);
    the full address lives where publishing happens. Revisit on complaints.
  - `/sites/config` currently carries one key; later deployment facts the
    web needs (e.g. AI availability for S1.28) can ride the same route
    additively.
- **Human inbox (unchanged, re-confirmed for this item):** production
  serving still needs the alo-sites container in compose (+`ALO_BLOB_DIR`
  mount), Caddy wildcard/on-demand-TLS routing `*.alosites.com` → alo-sites,
  and `SITES_DOMAIN=alosites.com` in both services' env — the domain itself
  is purchased and its DNS is live (see inbox top). Until that deploy, a
  "live" site is only reachable in local dev; the UI truthfully says where
  it WILL serve.
- **Next:** S1.16 (forms backend: public POST `/f/:form_id` on alo-sites,
  submissions store, internal-delivery notification, rate limit + tests).
