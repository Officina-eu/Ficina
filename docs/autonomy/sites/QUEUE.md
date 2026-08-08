# sites/QUEUE.md — alo Sites work queue (ADR 0036, track: SITES)

Ordered; the loop takes the first item not `[x]`/`[!]`. One item = one
iteration = one commit+push. Standard gates always (clippy/tests/tsc/eslint,
wrong-tenant on storage, curl wire-verification on the local backend). Detail
source: features.md → alo Sites. Code areas for THIS track: `platform/alo-store`
site_* modules, `products/sites/**`, `alo-jmap` `/sites/*` routes + module
registration, `web/src/sites/**`, `platform/alo-ai` sites generation module.
Do not touch billing/crm/business areas — that's the Mac's track.

## Wave S1 — site + blog + forms + both domain modes

- [x] S1.01 Design note `docs/design/sites.md`: data model (sites, pages, sections JSON versioning, themes, posts, forms, domains), render pipeline, the two-service boundary (edit API in alo-jmap vs public `alo-sites`), form flow, analytics privacy model, tenancy, out-of-scope. Done when: implement-skill's four blocks + the rejected alternative for the section model are written.
- [x] S1.02 Migration + store: `sites` (name, subdomain globally-unique + reserved-word list, status draft/live, theme JSON, created/updated) tenant-scoped CRUD + wrong-tenant tests + subdomain validation (dns-safe, 3–40 chars).
- [x] S1.03 Section schema v1 as typed Rust (serde) in a `site_model` module: nav, hero, features, text_image, gallery, testimonials, pricing, team, faq, cta, contact_form, footer — each with typed props + a `schema_version` envelope; exhaustive serde round-trip tests + golden fixture JSON per section.
- [x] S1.04 Migration + store: `site_pages` (slug, title, ordered sections JSON validated against S1.03, SEO meta, nav order, home flag) CRUD + slug rules + wrong-tenant tests.
- [x] S1.05 Theme model: palette+typography presets (≥6 shipped) + logo/favicon blob refs; validation + tests.
- [x] S1.06 Renderer: `products/sites/alo-sites` crate, `render` module — page JSON + theme → complete HTML document (semantic landmarks, alt required on images, meta/OG/canonical) — golden HTML tests per section type and a full-page golden.
- [x] S1.07 Stylesheet generation from theme tokens (one CSS file, responsive, no JS beyond menu toggle + form submit); byte-budget test (CSS < 50KB, page HTML < 100KB for the golden site).
- [x] S1.08 Publish flow in store: immutable per-page published snapshots + site publish state; republish creates new snapshot; tests prove drafts never leak to the published set.
- [x] S1.09 `alo-sites` public service: axum, resolves Host header (`<sub>.<SITES_DOMAIN>` env) → tenant site → serves published snapshots with in-memory cache + proper cache headers; 404 page; /healthz; in-process integration tests incl. Host isolation (site A's host can never serve site B).
- [x] S1.10 Edit API in alo-jmap: `/sites/*` — site CRUD, page CRUD, section ops (add/update/move/remove), theme set, publish — auth + Problem errors + wire transcript (401/422/happy paths) in sites/STATE.md.
- [x] S1.11 Web module skeleton: `web/src/sites` — rail entry (workspace surface), site list + create (name → live subdomain check), page list; i18n en.
- [x] S1.12 Web editor core: section stack (add from a picker with thumbnails, drag-reorder, delete) + per-type prop forms + save; tsc/eslint/build clean.
- [x] S1.13 Live preview: authenticated draft-render endpoint in alo-jmap reusing the render lib; iframe preview pane refreshing on save; mobile/desktop width toggle.
- [x] S1.14 Theme UI: preset picker + logo/favicon upload via Drive; preview updates.
- [x] S1.15 Publish UI: publish button with "goes live at <sub>.<domain>" copy, live/draft status chips; STATE human-inbox note: production needs the alo-sites container + wildcard DNS/TLS + SITES_DOMAIN purchase.
- [x] S1.16a Forms store ONLY: migration + `site_forms` (per-site, links a contact_form section token) + `site_form_submissions` (name/email/message, received_at, handled flag) + tenant-scoped CRUD + wrong-tenant tests. NO HTTP in this item. (Split after six oversized attempts — one turn must bank it.)
- [x] S1.16b Public submit endpoint: POST `/f/:form_id` on alo-sites — validation, size caps, honeypot field, per-IP rate limit — writing S1.16a submissions; in-process tests incl. rate-limit + a foreign form_id yielding a clean 404. NO notification yet.
- [ ] S1.16c1 Submission notification ONLY: on new submission, INTERNAL delivery of a notification message to the owner's inbox (never outbound SMTP) — build the RFC822, deliver via the existing store path, tests prove the message lands in the right tenant's inbox and a foreign tenant sees nothing. NO editor wiring in this item. (Split again 2026-08-08 03:45 after six hours of oversized attempts.)
- [ ] S1.16c2 Form auto-create on section add: adding a contact_form section in the editor creates/links the form record; wire transcript of the complete arc (add section → public POST → submission row → owner-inbox notification).
- [ ] S1.17a Submissions UI, list: per-site list with view + mark-handled; wire-verified.
- [ ] S1.17b Submissions UI, export: CSV export; wire-verified.
- [ ] S1.18 Blog model: `site_posts` (doc node ref, slug, title, excerpt, cover blob, published_at, status) + store + routes + tests (a post can only reference the tenant's own doc).
- [ ] S1.19a BlockNote→HTML core: paragraphs, headings, lists, quotes — goldens from real doc fixtures.
- [ ] S1.19b BlockNote→HTML rich: images, code, equations-fallback + the XSS-safety tests (script content never renders live).
- [ ] S1.20a Blog pages on alo-sites: post pages + /blog index cards; goldens.
- [ ] S1.20b Blog extras: pagination + RSS feed; goldens.
- [ ] S1.21a Blog UI, authoring: posts tab list + "write in alo Docs" creates/links the doc + edit opens the Docs editor.
- [ ] S1.21b Blog UI, publishing: publish flow with slug/cover/excerpt + status chips.
- [ ] S1.22a SEO, served: sitemap.xml + robots.txt on alo-sites; goldens.
- [ ] S1.22b SEO, edited: per-page meta editor UI + OG defaults from theme/logo; render goldens.
- [ ] S1.23 Privacy analytics collection on alo-sites: daily aggregates (path hits, referrer domain, unique-ish via daily-salted hash), explicitly NO ip/ua storage — a test asserts the stored schema contains no PII columns and raw request data is dropped.
- [ ] S1.24 Analytics UI: per-site panel (visits over time, top pages, top referrers) + the "no cookies, no banner" explainer string.
- [ ] S1.25a Custom domains, model half: `site_domains` migration + store (domain validation, TXT verify token generation, status pending/verified/live) + `/sites/{id}/domains` routes + the verify-check endpoint (DNS TXT lookup mockable for tests) + wrong-tenant tests + wire transcript.
- [ ] S1.25b Custom domains, serving half: alo-sites resolves verified custom Hosts + the Caddy on-demand-TLS "ask" endpoint (200 only for live domains/subdomains) + in-process Host-header tests; human-inbox note for the customer DNS how-to.
- [ ] S1.26a AI generation, envelope: `sites` module in alo-ai — full-site draft envelope (description → site JSON) with strict schema parse; deterministic fixture tests (NO live calls); prompt documents the section schema.
- [ ] S1.26b AI generation, repair: the one-retry repair path for near-miss outputs + its fixture tests.
- [ ] S1.27 AI edit ops (alo-ai): typed op envelope (add/remove/reorder section, set prop, rewrite copy) + apply-to-page pure fn + tests; ambiguous op → typed error the UI can surface.
- [ ] S1.28a Generation backend: POST `/sites/generate` (description → S1.26 envelope → draft site+pages created, never auto-published); unconfigured-AI path returns a typed "unconfigured" the UI can branch on; wire-verify fixture + unconfigured paths.
- [ ] S1.28b Generation UI: the "describe your business" onboarding screen → calls S1.28a → opens the editor on the draft; unconfigured path lands on blank-site + a template picker instead; build/tsc/eslint clean, manual click-path journaled.
- [ ] S1.29a Conversational editing, plumbing: per-page AI panel calling the ops endpoint; proposed ops as a human-readable change list; Approve applies / Discard; structural verify.
- [ ] S1.29b Conversational editing, preview: before/after page preview for a proposed op-set; approval-card polish.
- [ ] S1.30 AI copy tools per section (rewrite/tone/shorter/longer) as one generic op path + UI affordance on each text field; propose-then-approve.
- [ ] S1.30b Owner feedback (2026-08-07, from first hands-on test): the New-website dialog must show the FULL resulting address live as the user types (e.g. typing `axon` renders `axon.alosites.com — available`), not just a bare ADDRESS field with rules text; same full-address display wherever a subdomain is entered. Second finding from the same test: the user naturally typed the FULL domain (`axon.alosites.com`) and the UI showed generic "could not be checked/saved" while the server had returned a perfectly clear 422 detail ("subdomain may only contain lowercase letters, digits, and hyphens") — surface the server's detail message in the dialog, and strip/normalise an accidentally-typed suffix instead of failing. Third finding: the empty Address field left Create disabled with no hint why — the address must PRE-FILL from the site name as a slug (type "Axon" → suggests `axon`), editable but never blank by default, and a disabled Create must say what's missing. Done when: the dialog previews the complete address with availability state, shows server validation messages verbatim, typing the full domain just works, and the address self-suggests from the name.
- [ ] S1.30c Owner feedback (2026-08-07, hands-on test #2): after Create, the user landed on an empty Pages list and could not find "the editor" — creating a site must AUTO-CREATE the Home page and navigate STRAIGHT into the page editor (empty-state there invites "Add your first section"); the Pages list is for later, not the landing. Done when: Create → editor with Home open, one click from adding a hero.
- [ ] S1.31a Wave review, language: fr/nl strings for the whole sites UI; CHANGELOG sweep.
- [ ] S1.31b Wave review, reconciliation: docs/design/sites.md as-built; features [S1] reconciliation incl. the S1.30b/c dialog fixes; human-inbox summary (production compose+Caddy additions, AI key).
- [ ] S1.32a FINAL arc, forms+publish: fixture-generate → edit sections → theme → publish → serve on subdomain Host → form submission → owner-inbox notification + submissions UI; transcript.
- [ ] S1.32b FINAL arc, blog+domains+privacy: blog post from a real doc → custom-domain verify+serve → analytics counted with zero PII; transcript; then `LOOP COMPLETE`.
