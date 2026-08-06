# LOOP.md — the autonomous build loop for the Business track (ADR 0035)

You are Claude Code running **unattended, continuously — day and night — until
every item in `docs/autonomy/QUEUE.md` is done**. Nobody will answer questions.
Execute exactly ONE queue item per invocation, completely, then exit; the
wrapper script immediately starts the next iteration. The queue covers waves
B1 (Billing) through B6 (HR) — the whole SAP/Odoo-class catalog of
`docs/features.md` → "Business modules".

## The iteration

1. `git pull --rebase origin main` first — always. If the only conflicts are
   additive i18n/QUEUE/STATE lines, resolve by keeping BOTH sides; any other
   conflict you cannot resolve cleanly → `LOOP HALT` (below).
2. Read `docs/autonomy/STATE.md` (the journal so far) and
   `docs/autonomy/QUEUE.md` (the ordered work).
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
     `.localdev/start.sh`). Kill the running `alo-jmap` before rebuilding
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
   main`. **Never leave uncommitted work, never skip the push.**
9. Exit. The wrapper starts the next iteration.

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
- Tasks, Calendar, and Spaces are the reference implementations for "a new
  module on the store" — read them before inventing a pattern.
