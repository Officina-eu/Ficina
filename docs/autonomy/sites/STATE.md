# sites/STATE.md — Sites-track loop journal (append-only; newest at the bottom)

One entry per iteration: item id, what shipped, how verified, cuts/flags,
next item. The end-of-queue / emergency-stop control markers the wrapper
watches for are defined in LOOP.md — never write those exact phrases here
except to actually fire them.

Human-action inbox (things the loop must not do itself):

- Buy/choose the public sites domain (e.g. alosites.com) and set wildcard DNS
  — needed before any real subdomain goes live (ADR 0036 open decision).
- At next deploy: add the alo-sites container to production compose + Caddy
  wildcard/on-demand-TLS config (the loop never touches deploy/).
- Configure an AI provider key on the live server before real "generate my
  site" runs (loop verifies with fixtures only).

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
