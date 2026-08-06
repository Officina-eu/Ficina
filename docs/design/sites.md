# Design note — alo Sites (marketing site + blog + forms)

Status: building · 2026-08 · ADR 0036 · Sites track wave S1

alo Sites is the AI-native no-code website builder: "tell me about your
business" produces a complete draft site, then editing is conversational
(propose-then-approve, the ADR 0034 trust pattern) or manual through
typed section forms. V1 ships a marketing site + blog + contact forms,
published instantly at `<subdomain>.<SITES_DOMAIN>` and optionally on a
verified custom domain. This note records the data model, the render
pipeline, the two-service boundary, and the privacy posture before the
first migration lands; it is updated to as-built at the S1 wave review.

## Surface

- **Inputs (edit side):** authenticated workspace users driving
  `/sites/*` routes on `alo-jmap` — site CRUD, page CRUD, section
  operations (add / update / move / remove), theme selection, blog-post
  linking, publish, domain verification, form-submission review, and
  AI generation/edit envelopes (fixture-verified in the loop).
- **Inputs (public side):** anonymous browsers hitting the new
  **`alo-sites`** service (`products/sites/alo-sites`) — `GET` of
  published pages resolved by `Host` header, `POST /f/:form_id` for
  contact forms, `/blog` index + post pages + RSS, `sitemap.xml`,
  `robots.txt`, `/healthz`, and the Caddy on-demand-TLS "ask" endpoint.
- **Outputs:** complete static HTML documents (semantic landmarks,
  meta/OG/canonical) plus one theme-token-driven stylesheet and
  near-zero JS (menu toggle + form submit only); stored form
  submissions with an internal-mail notification; daily aggregate
  analytics rows.
- **Who calls it:** the web module `web/src/sites` (editor UI) calls
  `alo-jmap`; the public internet calls `alo-sites`; `alo-ai`'s sites
  module produces generation/edit envelopes that `alo-jmap` applies.

### Data model (tenant-scoped unless noted)

All tables carry `tenant_id` and are reached only through the
store's tenancy doors; ids are newtypes; timestamps are
`timestamptz`. New store modules are `site_*` files in
`platform/alo-store` (one file, one responsibility).

- **`sites`** — name, `subdomain` (**globally unique** across tenants,
  DNS-safe `[a-z0-9-]{3,40}`, checked against a reserved-word list:
  `www`, `mail`, `admin`, `api`, product names, …), status
  `draft | live`, theme JSON, created/updated. The subdomain column is
  the single deliberate cross-tenant surface: the claim check touches a
  global unique index but reveals only *taken / free*, never the owner.
- **`site_pages`** — site ref, `slug` (unique per site, `[a-z0-9-]`,
  empty allowed only for the home page), title, `sections` JSON
  (validated against the typed schema below on every write), SEO meta
  (title/description overrides), nav order, home flag.
- **Section JSON versioning** — a page's `sections` value is an
  envelope `{ "schema_version": 1, "sections": [ … ] }`. Each section
  is one variant of a typed Rust enum (serde, `#[serde(tag = "type")]`)
  in a `site_model` module: `nav`, `hero`, `features`, `text_image`,
  `gallery`, `testimonials`, `pricing`, `team`, `faq`, `cta`,
  `contact_form`, `footer` — each with typed props. Unknown section
  types or props are a **validation error on write** (the editor and AI
  are the only writers, both speak the schema) but tolerated as
  *skip-with-log on read* by the renderer, so an old renderer never
  500s on a newer snapshot mid-deploy. Version bumps ship an explicit
  pure upgrade function (v1 → v2) applied on read; stored JSON is
  rewritten lazily on the next save. Prices inside the `pricing`
  section are **display strings**, not money values — nothing computes
  on them, so the integer-cents law is not in play.
- **`site_page_snapshots`** — immutable per-page published copies:
  page ref, snapshot of sections + meta + slug + nav order, theme
  snapshot on the site's publish record, `published_at`. Publish
  creates new snapshot rows and flips the site's published-set pointer
  atomically; **the public service reads only snapshots**, so drafts
  are unreachable from the internet by construction, not by filtering.
- **Themes** — a JSON value on `sites`: palette + typography preset id
  (≥ 6 shipped presets) plus optional logo/favicon blob refs
  (tenant blobs via the existing Drive/blob store). Validated against
  a typed theme struct on write.
- **`site_posts`** (blog) — site ref, doc node ref (**must reference
  the tenant's own doc** — enforced in the store query, tested),
  slug, title, excerpt, cover blob ref, `published_at`, status
  `draft | published`. Post bodies live in alo Docs; publishing renders
  BlockNote JSON → HTML through a dedicated renderer with an
  XSS-safety test (script/style/event-handler content never reaches
  the published HTML).
- **`site_forms` / `site_form_submissions`** — a `contact_form`
  section references a form id; submissions store the posted fields
  (size-capped), `received_at`, and handled flag. **No IP and no
  user-agent are ever stored.**
- **`site_domains`** — domain name, TXT verification token, status
  `pending | verified | live`. Serving on a custom Host requires
  status `verified`+; the Caddy on-demand-TLS "ask" endpoint answers
  from this table.
- **`site_analytics_daily`** — (tenant, site, date, path, referrer
  domain, hit count, unique-ish count). Uniqueness is a
  **daily-salted hash** kept only in memory for the day; the stored
  schema has **no PII columns** — a test asserts the column list, and
  raw request data (IP, UA, full referrer URL) is dropped after the
  in-memory aggregation step.

### Render pipeline

```
page JSON (typed sections) + theme JSON
        │  validate/upgrade to current schema_version
        ▼
render lib (products/sites/alo-sites, `render` module — pure fns)
        │  section renderer per type → semantic HTML fragments
        ▼
full document: <head> meta/OG/canonical + landmarks + footer
        +  one generated stylesheet from theme tokens (CSS < 50 KB,
           page HTML < 100 KB for the golden site — byte-budget tests)
```

The renderer is a **library first**: `alo-sites` serves it publicly
from snapshots; `alo-jmap` reuses the same library for the
authenticated draft-preview endpoint, so preview and production HTML
cannot drift. Golden-HTML tests per section type plus a full-page
golden pin the output.

### Two services, one boundary

- **`alo-jmap`** (existing, authenticated): all editing/management —
  tenant-scoped like every module, `Problem` errors, routes registered
  in `server.rs`. It never serves public traffic.
- **`alo-sites`** (new, `products/sites/alo-sites`): the only
  unauthenticated surface. Resolves `Host` → (tenant, site) via one
  indexed lookup (subdomain of `SITES_DOMAIN`, or a verified custom
  domain), then serves **published snapshots only**, with an in-memory
  cache + correct cache headers, a branded 404 page, and `/healthz`.
  It accepts form POSTs and the analytics tick — nothing else writes.
  Host isolation (site A's host can never serve site B's content) is
  an in-process integration test, not an assumption.

Dependency direction stays legal: `products/sites` depends on
`platform/alo-store` and never on another product; the web editor
talks only to `alo-jmap`.

### Form flow

```
visitor → POST /f/:form_id on alo-sites
  → size caps + honeypot field (silent drop) + per-IP rate limit
    (in-memory sliding window; the IP is used transiently and never
    persisted)
  → insert into site_form_submissions (tenant-scoped via the form's
    site)
  → notification by INTERNAL delivery to the owner's inbox (the
    existing local-delivery path — never outbound SMTP)
  → CRM lead creation when the business track's B2 lands (out of
    scope here; the seam is the submission row)
```

### AI posture (S1.26–S1.30)

Generation and editing are structured envelopes in `alo-ai`'s sites
module: full-site draft (business description → site JSON) and typed
edit ops (add/remove/reorder section, set prop, rewrite copy) with
strict schema parse + one repair retry. Everything is
propose-then-approve; a draft site is never auto-published. The loop
verifies with **fixture model outputs only** — live calls require a
human-configured key, and the unconfigured path degrades to
blank-site + templates.

## Errors

Edit side (`alo-jmap`, RFC 9457 `Problem` bodies like every module):

- Unauthenticated → `401`; authenticated but wrong tenant/user →
  the id simply does not resolve → `404` (the account-door pattern:
  wrong-tenant is indistinguishable from nonexistent).
- Validation (bad subdomain, reserved word, slug collision, section
  JSON failing the typed schema, oversized theme/logo, post doc ref
  outside the tenant) → `422` with a field-level detail.
- Subdomain taken (any tenant) → `422 subdomain_taken` — taken/free
  only, no owner information.
- Publish with zero pages / no home page → `422`.
- AI envelope that fails schema parse after one repair retry → typed
  error the UI surfaces as "couldn't apply, nothing changed".

Public side (`alo-sites` — terse, static, no internals on the wire):

- Unknown host or unpublished site → branded `404` page (no tenant
  existence leak: unknown subdomain and unpublished site are
  identical).
- Unknown path on a live site → the site's `404` page.
- Form: unknown form id → `404`; body over size cap → `413`;
  malformed → `400`; rate-limited → `429` with `Retry-After`;
  honeypot tripped → `200` (silent drop — bots learn nothing).
- TLS "ask" endpoint: unverified domain → non-200, so Caddy never
  issues a certificate for a domain we haven't verified.

## Tenancy

- Every `site_*` table carries `tenant_id`; store access goes through
  the existing doors (`for_tenant` / `for_account`) so the scope
  predicate is baked into every statement — wrong-tenant reads return
  `NotFound`/empty, never data, never 500. **Wrong-tenant tests are
  mandatory on every `site_*` store module** (the queue repeats this
  per item).
- The public service holds no session: its tenant scope is derived
  **from the Host lookup result** — one global indexed read maps host
  → (tenant, site), and every subsequent read (snapshots, posts,
  forms, analytics) is scoped by that pair. The Host-isolation
  integration test proves site A's host cannot serve site B.
- Deliberate global surfaces, and the only ones: the `subdomain`
  unique index (leaks taken/free only) and the host→site resolver
  (public data by definition). Everything else — pages, snapshots,
  posts, submissions, domains, analytics — is tenant-scoped.
- Form submissions and analytics write **into** a tenant's scope from
  anonymous traffic; the writable set is exactly {insert submission,
  bump aggregate} for the resolved site — no read-back surface exists
  publicly.

## Out of scope (v1 — cuts are decisions)

- E-commerce checkout, catalog storefront, booking pages (S+ / ADR
  0035's later waves).
- Free-form design tools / pixel canvas, custom code injection,
  template marketplace, third-party embeds or trackers of any kind
  (ADR 0036 non-goals; the analytics promise depends on it).
- Version history + rollback UI, scheduled publishing,
  password-protected pages, whole-site AI translation, responsive
  image derivatives (S2 — snapshots are already immutable so rollback
  has its substrate).
- CRM lead creation from form submissions (waits for business-track
  B2; the seam is the stored submission).
- Production serving infrastructure: the `alo-sites` container,
  wildcard DNS/TLS, and the `SITES_DOMAIN` purchase are human deploy
  actions recorded in the sites STATE human-inbox — the loop never
  touches `deploy/`.
- Multi-site-per-tenant limits, quotas, and billing integration —
  unlimited sites per tenant in v1; revisit with billing.

**Rejected alternative (section model):** a free-form block/canvas
model (arbitrary nested layout tree, absolute positioning) was
rejected because the AI cannot reliably read or edit it, every AI
change would be an un-reviewable pixel diff, and rendering it fast
and accessible is a research project — typed sections give the model
a closed vocabulary, give users a form-based editor, and give the
renderer a finite, golden-testable surface (ADR 0036).

**Rejected alternative (serving):** serving public traffic from
`alo-jmap` behind a path prefix was rejected because it would put an
unauthenticated, internet-facing surface inside the workspace API
process — a separate `alo-sites` binary keeps the blast radius, cache
behavior, and scaling profile of anonymous traffic away from tenant
data paths.
