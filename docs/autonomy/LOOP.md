# LOOP.md — the autonomous build-loop protocol (multi-track)

You are Claude Code running **unattended, continuously — day and night — until
every item in your track's QUEUE is done**. Nobody will answer questions.
Execute exactly ONE queue item per invocation, completely, then exit; the
wrapper script immediately starts the next iteration.

## Tracks (the invocation prompt names yours)

| Track | Queue / journal | Code areas it owns |
|---|---|---|
| **business** (default) | `docs/autonomy/QUEUE.md` + `STATE.md` | billing/crm/projects/finance/inventory/hr: store modules, `/billing`- etc. routes, `web/src/billing…` (ADR 0035, waves B1–B6) |
| **sites** | `docs/autonomy/sites/QUEUE.md` + `sites/STATE.md` | `site_*` store modules, `products/sites/**`, `/sites/*` routes, `web/src/sites/**`, alo-ai sites module (ADR 0036) |

Two loops run in parallel on different machines, both pushing to `main`.
**Never touch the other track's areas.** The deliberately-shared files
(`i18n/en.ts`, `CHANGELOG.md`, route registration in `server.rs`, `mod`
lines in `lib.rs`) only ever receive ADDITIVE lines from either track — on a
rebase conflict there, resolve by keeping BOTH sides; any non-additive
conflict you cannot resolve cleanly → `LOOP HALT`.

## The iteration

1. `git pull --rebase origin main` first — always. If the only conflicts are
   additive i18n/QUEUE/STATE lines, resolve by keeping BOTH sides; any other
   conflict you cannot resolve cleanly → `LOOP HALT` (below).
2. Read your track's STATE (the journal so far) and QUEUE (the ordered
   work) — paths from the Tracks table above.
3. Pick the FIRST item that is neither `[x]` done nor `[!]` blocked.
   - All items done → append `LOOP COMPLETE` to STATE.md, commit, push, exit.
   - Only blocked items remain → re-attempt the OLDEST `[!]` item once with
     fresh eyes; if it fails again, append `LOOP COMPLETE (with blockers)` and
     exit — a human unblocks.
4. Build that ONE item at **full depth** (CLAUDE.md laws + the `implement`
   skill): input → validation → logic → persistence → output → error paths.
   Cut scope, never depth — a narrower slice that fully works beats the listed
   slice half-done (record any cut in STATE.md).
5. Gate it (all must pass):
   - Rust touched: `cargo fmt` on changed crates; `SQLX_OFFLINE=true cargo
     clippy -p <crates> --all-targets` clean; `cargo test -p <crates>` green.
   - Web touched: `npx tsc --noEmit`; `npx eslint <changed files>`;
     `npm run build` — all clean.
   - Storage touched: the **wrong-tenant test is mandatory** (tenant A
     reaching tenant B's record gets a clean denial, proven by a test).
   - New HTTP routes: wire-verify against the LOCAL backend — docker postgres
     (`alo-pg`, user/db `alo`, password `alo-dev-only`) + the debug
     `alo-jmap` binary (`DATABASE_URL=postgres://alo:alo-dev-only@localhost:5432/alo`,
     `ALO_BLOB_DIR=<repo>/.localdev/blobs`, `ALO_JMAP_ADDR=127.0.0.1:8080`,
     `ALO_IDENTITY_ISSUER=http://localhost:5173`; bootstrap once with
     `identityctl bootstrap-admin` + `register-client web` as in
     `.localdev/start.sh`). ALWAYS kill any running `alo-jmap` BEFORE
     starting your test server — not only before rebuilding: a stale server
     from a killed iteration squats the port for days and fails every bind
     (found: one survived 19 hours and poisoned a whole night). Kill it before rebuilding too
     (macOS/Linux: `pkill -f alo-jmap`; Windows locks the exe:
     `taskkill //F //IM alo-jmap.exe`). Real curl calls, real DB rows checked.
6. Update what changed behaviour: a CHANGELOG.md line (user voice), rustdoc/
   TSDoc on public items, all UI strings through `i18n/en.ts` (fr/nl at wave
   reviews). New top-level route prefixes: note in STATE.md that the
   production Caddyfile needs the prefix added at next deploy (do NOT touch
   deploy/ yourself).
7. Mark the item `[x]` in QUEUE.md. Append one STATE.md entry: item id, what
   shipped, how verified, cuts/flags, next item id.
8. Commit (conventional message + the Co-Authored-By line), `git push origin
   main`. **Never leave uncommitted work, never skip the push.** If the push
   is rejected (the other track pushed first), `git pull --rebase` (keep-both
   on the additive shared files) and push again — retry up to three times,
   then `LOOP HALT` rather than leave the item unpushed.
9. Exit. The wrapper starts the next iteration.

## Headless discipline (learned the hard way)

You run as a single non-interactive invocation: **when your turn ends, you
cease to exist** — there is no "later". Therefore: NEVER background a build,
test, or any gate command and end your turn waiting for it; run gates in the
foreground and wait for their real exit codes inside the same turn. An ended
turn with uncommitted work means the work is DISCARDED by the wrapper. If the
remaining gates cannot finish in this iteration's budget, cut the item's
scope (LOOP rule: cut scope, never depth), gate the narrower slice fully,
commit and push it, and journal the remainder as the next item's starting
point.

## If stuck

- Two honest failed attempts at a gate → mark the item `[!] blocked: <one
  line>` in QUEUE.md, details in STATE.md, commit, push, exit. The loop moves
  on. Never thrash for hours; never ship a stub to get past a gate.
- Environment broken (docker down, disk full, unresolvable conflict) →
  append `LOOP HALT: <reason>` to STATE.md, commit if possible, exit non-zero.
  The wrapper stops on HALT; a human restarts after fixing.

## Hard safety rails (absolute — no exceptions, ever, unattended)

- **Never touch production**: no ssh, no deploys, nothing at 152.53.179.142 or
  any *.alomails.com / *.aloworkplace.com host. Build + local-verify + push ONLY.
  Deploys happen only when the human is present.
- **Never send real email; never call paid/external AI APIs.** Agent-tool
  slices are verified structurally (routes exist, 401/422 guards, execute
  against the local DB) — never by live model calls.
- **Never commit secrets**, keys, `.env`, `.localdev/` contents, or memory
  files — the repo is PUBLIC. The pre-commit secret hook stays green.
- **Never** force-push, rewrite history, delete branches, edit `deploy/`,
  `.github/`, or others' ADRs. Your write scope is: the code for the current
  item + its tests + migrations + QUEUE/STATE/CHANGELOG/docs for that item.
- Migrations are append-only new files, expand-only — no destructive DDL.
- Legal/compliance items (gapless numbering, VAT, EN 16931 e-invoices):
  implement the strict reading of the cited spec; flag any ambiguity in
  STATE.md for human review — never guess loosely on compliance.

## Standing context

- Vision/scope: ADR 0035; `docs/features.md` → Business modules; ROADMAP →
  Business track. Nothing outside the queue gets built.
- Architecture: `platform/alo-store` = tenant-scoped store (`for_account`,
  newtype ids, `thiserror`); `products/mail/alo-jmap` = axum routes
  (`Problem` errors, `authenticate`, register in `server.rs`); `web/src` =
  React, i18n catalogs, ds tokens, module pattern like Tasks/Calendar.
- Money is ALWAYS integer cents (i64); VAT rates in basis points; totals
  computed server-side; never floats for money anywhere.
- **Every UI slice obeys `docs/design/ux-principles.md`** (the interface
  laws: zero-manual, recognition over recall, empty-states-as-onboarding,
  undo over confirm, verbatim helpful errors, tokens-only styling). Wave
  reviews test the laws on each new screen and file violations as items.
- Tasks, Calendar, and Spaces are the reference implementations for "a new
  module on the store" — read them before inventing a pattern.
