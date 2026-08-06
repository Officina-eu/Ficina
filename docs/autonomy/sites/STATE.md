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
