# ADR 0019 — Platform + product repos (the suite structure)

Status: **accepted** (owner-approved) · Stage 1 landed 2026-08-03

## Update — Stage 1 landed

The monorepo is now layered `platform/ products/mail/ suite/` with the
one-way dependency direction enforced by the Cargo path graph. Verified:
`cargo check --workspace` green, and all four service images
(alo-jmap/smtp/imap/control) rebuild from the new Dockerfile/compose paths
(alo-jmap redeployed healthy). No behaviour change — pure structure.

**One placement refinement vs. the draft:** `alo-ai` sits in **`platform/`**,
not the suite. Its egress/SSRF guard (`alo_ai::egress`) is shared
infrastructure the Mail product already depends on (IMAP-import host
validation), so it cannot be suite-only without breaking the product build.
Carving the *proprietary AI features/models* from the shared inference
plumbing is a later refinement and does not block the split.

## Update — Stage 2 landed (`aloworld-org/alomails` seeded)

The public Mail product repo is live at
[`aloworld-org/alomails`](https://github.com/aloworld-org/alomails)
(AGPL-3.0). Content: `platform/` + `products/mail/` + `migrate/` + a mail-only
workspace `Cargo.toml` + `deploy/production/` (the `alo-control` service and
its Caddy route removed) + `LICENSE` + `README`. Verified: `cargo check
--workspace` green **standalone** in the seeded tree.

**Deviation from the draft — clean seed, not a subtree with history.** The
monorepo's history carries private business/licensing strategy (ADR 0002,
control-plane ADRs, positioning) that touched shared paths; a filtered-history
export would leak those commit messages/contents into a public repo. So the
seed is a **single clean initial commit**. The trade-off: `alomails` does not
(yet) share commit history with the monorepo. Development still happens in the
monorepo; `alomails` is refreshed from it. Curated history can be added later
if wanted.

**Web client — shipped, and modularized (zero-divergence).** The web app is now
defined by a single **product surface** (`web/src/product`): which rail
modules, full-screen consoles, and compose-editor inserts a product has.
App.tsx, the rail, the account menu, and the rich-text editor read the surface
generically — none hard-imports `control`/`authoring`. A `@product` build alias
(vite `ALO_PRODUCT` + tsconfig path) selects the surface: **workspace** by
default, **mail** with `ALO_PRODUCT=mail`. Verified from one source: the default
build carries Docs (59 KaTeX assets); the mail build carries **zero** KaTeX and
no control/authoring chunks. tsc + eslint clean, 73 web tests pass.

Only `product/workplace.tsx` imports the suite-only areas, so exporting alomails
is now **mechanical**: copy the workspace web, delete `src/control` +
`src/authoring` + `product/workplace.tsx`, default to mail — no logic edits.
alomails already runs this structure (`alomails/web`, mail surface). The web is
thus a **trimmed export with a one-file seam**, not a hand-maintained fork.

**Still deferred:** the Stage-3 flip to versioned platform releases once the
`alo-store`/`alo-identity` contracts stabilise.

## Context

alo is becoming a suite: Mail first, then Docs, Calendar, Chat, Meet,
Drive. The owner wants each product to be its **own repo/product**
(e.g. `aloworld-org/alomails`), joined into a workspace — the
Microsoft 365 / Google Workspace shape — and wants the structure that
is best **long-term as repos and clients grow**, not a shortcut.

The constraint that shapes everything: the dependency graph shows the
products share almost all of the Rust code. Everything in `core/` is
mail/foundation **except `alo-control`**; `alo-jmap` alone serves mail
JMAP *and* the OIDC provider, admin, contacts, signup. So a physical
**fork** of "the mail code" would duplicate `alo-store`,
`alo-identity`, `alo-jmap`, … and force double-maintenance — the exact
waste to avoid.

The M365/Workspace precedent resolves this: Gmail, Docs and Drive are
separate *products* but share **one platform** — a single identity/SSO,
storage substrate and tenancy model (Microsoft's "Graph"). Products are
independent; the platform beneath them is shared, not copied.

This also aligns with [ADR 0002](0002-agpl-dual-license.md): an AGPL-3.0
open core with a proprietary control plane. Public, independently
deployable product repos ARE the "sovereignty made verifiable" pitch.

## Decision

Adopt a **three-layer structure**, mapped to repos as the seams harden:

1. **Platform (`alo-platform`)** — the shared kernel every product builds
   on: identity/tenancy (`alo-identity`), the storage substrate
   (`alo-store`), and shared libraries (`alo-auth-mail`, `alo-sieve`).
   Its public surfaces are **stable, versioned contracts** — the whole
   structure depends on them not churning.
2. **Products (`alomails`, later `alodocs`, `alocalendar`, …)** — each its
   own repo, its own AGPL release cadence, **deployable standalone**,
   depending on a *pinned* platform version. `alomails` = `alo-smtp`,
   `alo-smtp-client`, `alo-imap`, `alo-jmap` + the mail web app.
3. **Suite (`alo-workplace`, this repo)** — the umbrella that *composes*
   products into one experience: SSO across products, unified shell,
   cross-product search/AI, plus the proprietary control plane and
   billing. Depends on the products + platform.

**Dependency direction is one-way and enforced:** suite → products →
platform. A product never depends on the suite or on another product;
shared behaviour lives in the platform.

Code is **never duplicated across repos.** The shared 90% is a
*dependency*, owned once in the platform.

## Staged migration (no big-bang rewrite; friction added only as it pays off)

**Stage 1 — draw the layers inside this monorepo (now).** Reorganise into
`platform/`, `products/mail/`, `suite/` and enforce the one-way
dependency direction (products → platform, suite → products). Feature-gate
suite-only bits out of the product build (`alo-jmap`'s `/ai/*` behind an
`ai` feature; the mail web app excludes `authoring`/`control`). Pure
moves + flags — zero cross-repo friction, and it makes the boundaries
real and testable. This is the prerequisite for any clean split.

**Stage 2 — publish `alomails` as a real repo (now/soon).** Seed
`aloworld-org/alomails` from `platform/ + products/mail/ + mail web` as a
**git subtree** — a genuine, buildable, deployable AGPL repo that can be
pushed to *and* pulled from, so independent deployment and outside
contributions work — while development still happens in one place until
the platform contracts settle. History preserved.

**Stage 3 — independent versioning (when platform contracts stabilise).**
Promote the platform to versioned releases (git tags, later published
crates); flip each product to consume a *pinned* platform version. The
product repos become fully independent source-of-truth with their own
cadence. The workplace pins product + platform versions like any
suite integrator.

The trigger for Stage 3 is **contract stability**, measured by how often
a change to `alo-store`/`alo-identity` public surfaces breaks a product —
not a date. Splitting before then just front-loads two-repo pain.

## Consequences

- **No rewrite, ever** — the shared foundation is a dependency, not a copy.
- **Products can ship and be audited independently** (the AGPL story) while
  the suite stays a thin integrator.
- **The cost is contract discipline**: the platform's APIs become
  first-class versioned contracts (expand → migrate → contract), because
  multiple repos will pin them. This is a feature — it forces the
  stability the constitution already demands.
- **Cross-repo friction is real but deferred** to Stage 3, and bounded by
  the one-way dependency rule.
- One-time work: the crate reorg (Stage 1) and the subtree/export tooling
  (Stage 2). Small and mechanical next to a fork-and-maintain-twice.

## Alternatives rejected

- **Fork the mail code into `alomails`** — duplicates `alo-store`/
  `alo-identity`/`alo-jmap`; double-maintenance forever. This is the
  option the owner explicitly wants to avoid.
- **Monorepo → one-way export mirror only** — cheapest, but the product
  repo is a downstream artefact, not a real product with its own life;
  weaker open-source story and doesn't scale to many products/clients.
- **Sever all repos immediately** — "correct" end state applied too early;
  front-loads cross-repo friction while the platform contracts are still
  changing daily, slowing us down when we are moving fastest.
