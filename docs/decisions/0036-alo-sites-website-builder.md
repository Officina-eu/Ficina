# 0036 — alo Sites: the AI-native no-code website builder

Date: 2026-08-06 · Status: accepted

## Decision

alo gains **alo Sites**: a no-code website builder where **AI is the starting
point** — "tell me about your business" produces a complete draft site
(structure, sections, real copy), then editing is conversational
(preview-then-approve diffs, the ADR 0034 trust pattern) or manual. It
completes the SME bundle no one else in Europe offers in one sovereign place:
**domain → email → website → leads (CRM, wave B2) → invoices (B1)** — and it
is Odoo's proven acquisition wedge, rebuilt on our stack.

V1 scope (user decision): **marketing site + blog + forms**, with **both**
instant subdomain publishing (`<name>.<sites-domain>`) and **custom domains**
(reusing the mail DNS-onboarding flow). E-commerce checkout stays a later
wave (ADR 0035's list).

## Architecture

- **Section-based, not free-form.** A page is an ordered list of typed
  sections (hero, features, pricing, team, FAQ, contact…) stored as a
  **versioned JSON model** with typed Rust schemas. This is the AI-native
  choice: the model reads and edits structured JSON reliably, and every AI
  change renders as a previewable diff. No pixel canvas, ever.
- **Static-first rendering in Rust.** Model + theme → plain fast HTML + one
  token-driven stylesheet, near-zero JS. No WordPress-class attack surface;
  millisecond loads; SEO-correct by construction.
- **Two services, one boundary.** Editing/management API lives in `alo-jmap`
  (authenticated, tenant-scoped, like every module). Public serving is a new
  **`alo-sites`** service (products/sites): serves published snapshots by
  Host header (subdomain + custom domains, Caddy on-demand TLS), handles
  form POSTs — the only unauthenticated surface, kept away from the
  workspace API.
- **Blog = alo Docs.** Posts are written in the existing Docs editor; a
  BlockNote-JSON → HTML renderer publishes them. One editor, one brain.
- **Forms** store submissions tenant-side, notify by internal delivery to
  the owner's inbox, and (once B2 lands) create CRM leads. No third parties.
- **Privacy-first analytics**: aggregate daily counts (path, referrer
  domain), salted-hash dedupe, **no IP stored, no cookies, no consent banner
  needed** — a sales argument, enforced by tests.

## AI posture

Generation and edit operations are structured envelopes (like the agent):
full-site JSON draft, section ops (add/remove/reorder/set-prop/rewrite-copy),
SEO metadata, translations later. All propose-then-approve; EU models. The
build loop verifies AI features **structurally with fixture outputs** — live
model calls only when a human wires a key.

## Execution

Built by a second autonomous loop (the **Sites track**) running on the office
PC in parallel with the Mac's Business track — the loop machinery becomes
multi-track (per-track QUEUE/STATE, disjoint code areas, keep-both rule on
shared additive files). Queue: `docs/autonomy/sites/QUEUE.md`.

## Non-goals (v1)

Free-form design tools, custom code injection, template marketplace,
e-commerce checkout, third-party embeds/trackers of any kind. The sites
domain itself (e.g. `alosites.com`) is a human purchase — open decision.
