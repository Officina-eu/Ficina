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
- [ ] S1.16 Forms backend: contact_form section wiring — public POST `/f/:form_id` on alo-sites (per-IP rate limit, honeypot field, size caps), `site_form_submissions` store, notification by INTERNAL delivery to the owner's inbox (never outbound SMTP); tests incl. rate-limit + wrong-tenant.
- [ ] S1.17 Submissions UI: per-site list with view/mark-handled + CSV export; wire-verified.
- [ ] S1.18 Blog model: `site_posts` (doc node ref, slug, title, excerpt, cover blob, published_at, status) + store + routes + tests (a post can only reference the tenant's own doc).
- [ ] S1.19 BlockNote-JSON → HTML renderer (paragraphs, headings, lists, quotes, code, images, equations fallback-to-text) with golden tests from real doc fixtures; XSS-safety test (script content never renders live).
- [ ] S1.20 Blog rendering on alo-sites: /blog index (cards, pagination) + post pages + RSS feed; goldens.
- [ ] S1.21 Blog UI: posts tab — "write in alo Docs" creates/links a doc, edit opens the Docs editor, publish flow with slug/cover/excerpt.
- [ ] S1.22 SEO pack: sitemap.xml + robots.txt on alo-sites, per-page meta editor UI, OG defaults from theme/logo; goldens.
- [ ] S1.23 Privacy analytics collection on alo-sites: daily aggregates (path hits, referrer domain, unique-ish via daily-salted hash), explicitly NO ip/ua storage — a test asserts the stored schema contains no PII columns and raw request data is dropped.
- [ ] S1.24 Analytics UI: per-site panel (visits over time, top pages, top referrers) + the "no cookies, no banner" explainer string.
- [ ] S1.25 Custom domains: `site_domains` (domain, TXT verify token, status pending/verified/live) + verify check endpoint + serving by verified custom Host on alo-sites + Caddy on-demand-TLS "ask" endpoint; local wire-verify with Host headers; human-inbox notes for real DNS docs.
- [ ] S1.26 AI generation (alo-ai): `sites` module — full-site draft envelope (business description → site JSON: pages, sections, copy) with strict schema parse + one repair retry; deterministic tests on fixture model outputs (NO live calls); prompt documents the section schema.
- [ ] S1.27 AI edit ops (alo-ai): typed op envelope (add/remove/reorder section, set prop, rewrite copy) + apply-to-page pure fn + tests; ambiguous op → typed error the UI can surface.
- [ ] S1.28 Generation flow (jmap + web): "describe your business" onboarding → POST /sites/generate → draft site created (never auto-published) → editor opens; unconfigured-AI path degrades to blank-site + templates; wire-verify the unconfigured + fixture paths.
- [ ] S1.29 Conversational editing UI: per-page AI panel — request → proposed ops rendered as a human-readable change list + before/after preview → Approve applies / Discard; reuses the approval-card pattern; structural verify.
- [ ] S1.30 AI copy tools per section (rewrite/tone/shorter/longer) as one generic op path + UI affordance on each text field; propose-then-approve.
- [ ] S1.31 Wave review: fr/nl strings for the sites UI, CHANGELOG sweep, docs/design/sites.md as-built, features [S1] reconciliation, human-inbox summary (sites domain purchase, production compose+Caddy additions, AI key).
- [ ] S1.32 FINAL integration arc on local: fixture-generate a site → edit sections → theme → publish → serve on subdomain Host → form submission → owner-inbox notification + submissions UI → blog post from a real doc → custom-domain verify+serve → analytics counted with zero PII — full transcript in sites/STATE.md, then `LOOP COMPLETE`.
