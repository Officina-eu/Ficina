# STATE.md — loop journal (append-only; newest at the bottom)

The loop appends one entry per iteration: item id, what shipped, how it was
verified, anything cut or flagged for human review, and the next item. Humans
read this file with morning coffee; the loop reads it to regain context.
The end-of-queue / emergency-stop control markers the wrapper watches for are
defined in LOOP.md — never write those exact phrases here except to actually
fire them.

Human-action inbox (the loop adds items here it must not do itself —
deploys, Caddyfile prefixes, Peppol account, AI-provider keys):

- **Caddyfile prefix at next deploy:** `/billing` is a new top-level route
  prefix (design note B1.01). The production Caddyfile needs it added when
  B1's routes actually ship (B1.05 onwards). The loop never edits `deploy/`.
- **rustfmt divergence between the two machines.** `main` is not
  `cargo fmt --check` clean under rustfmt 1.9.0 (style edition 2024, which
  reorders `use` groups and re-wraps struct literals). Running `cargo fmt`
  on the Mac reformats hundreds of pre-existing lines in any crate it
  touches, which would bury real diffs and collide with the sites track.
  Until a human pins one toolchain for both machines (a `rust-toolchain.toml`
  is the usual fix), iterations on this Mac should format only the lines
  they add rather than running `cargo fmt` across a crate.

---

marathon preflight from the Mac, 2026-08-06 — toolchain and push access verified.

## 2026-08-06 — baseline (pre-B1.01): the suite was not green on unix

Before starting the queue, `cargo test --workspace` was run on macOS for the
first time and had **four** failures, none of them in product code:

- `sieve_redirect_is_arc_sealed_and_validates` and
  `per_tenant_key_signs_and_validates_not_the_file_key` — both wrote a key
  PEM with `fs::write` (mode 0644 under the default umask). The keystore
  correctly refuses a group/world-readable private key on unix, so sealing
  silently produced nothing; on Windows that permission check is a no-op,
  which is why these passed there. Fixed by chmod 0600, matching what the
  in-crate `alo-auth-mail` tests already do.
- `deleting_a_tenant_purges_its_tasks` — asserted through `task_projects()`,
  which first *ensures* the personal project exists; that write cannot
  succeed for a deleted tenant, so it failed on the foreign key rather than
  returning empty. This one had been failing on `main` since 2026-08-04 on
  every platform — CI never reported it because the CI queue is backed up
  and no run has completed. Fixed by asserting on the stored rows.
- `rspamd_runs_and_stamps_without_a_resolver` and
  `check_talks_to_a_loopback_endpoint` — canned HTTP stand-ins drained the
  request with one `read` then closed, so unread bytes made the kernel send
  RST instead of FIN and the client saw "connection reset". Extracted into
  `alo-smtp/src/canned_http.rs`, which reads the request in full.
- Also found while chasing the above: `submission_tls.rs` shared one
  `PgPool` across six `#[tokio::test]` runtimes, so every AUTH test after
  the first hung to its 10s timeout. `alo-store`'s own harness documents
  this exact rule. The store is now built per test.

Verified: clippy clean workspace-wide, `cargo test --workspace` green
(626 passed) on three consecutive full runs, plus 8 repeat runs of the
previously flaky `submission_tls` suite. Commit `f7c4ee6`.

## 2026-08-06 — B1.01 billing design note

Shipped `docs/design/billing.md`: the B1 surface (the `/billing/*` route
table and who calls it), the `billing_*` data model (customers, products,
quotes, invoices + lines, payments, sequences) with money as integer cents
and VAT in basis points, the totals function with rounding at the VAT-rate
subtotal, the full error map from `StoreError` to HTTP, the tenancy story
(`for_account` as the only door; wrong-tenant is `404`, never an existence
oracle), and the out-of-scope list.

Numbering decision recorded with its rejected alternative, as the item's
"done when" required: a row-locked `billing_sequences` row inside the
issuing transaction, **rejecting** a Postgres `SEQUENCE`/`nextval()`
because sequences are non-transactional — a rolled-back issue burns a
number and leaves a permanent gap, which EU gapless-numbering law does not
allow.

Verified: docs-only change, so no code gates apply; the workspace clippy
and test gates above were green at the same commit. No cuts.

Flagged for a human: the `/billing` Caddyfile prefix and the rustfmt
divergence, both in the inbox above.

Next item: B1.02 (migration + store for `billing_customers`).
