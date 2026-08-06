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

## 2026-08-06 — note before B1.02: migration numbering across the two tracks

A first attempt at B1.02 was started and then aborted (the checkout was being
renamed from the retired "Ficina" name to `alo-workplace`); nothing was
committed and B1.02 is untouched in the queue. One observation from that
attempt is worth keeping, because it prevents a real collision:

**The business track mints migrations in the `01xx` block; the sites track
continues in `00xx`.** The two loops run on different machines and cannot see
each other's uncommitted work, so picking "the next number after the highest
one I can see" makes both tracks eventually choose the same version — and two
different migrations sharing a version is a broken schema, not a merge
conflict. Sites is at `0056`; business starts at `0100`.

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

## 2026-08-06 — B1.02 billing customers (migration + store)

Shipped the first billing table and its store module:

- **Migration `0100_billing_customers.sql`** — the first migration in the
  business track's `01xx` block (sites continues in `00xx`). Tenant-scoped,
  `PRIMARY KEY (tenant_id, id)`, `REFERENCES tenants(id) ON DELETE CASCADE`,
  a named FK `billing_customers_contact_fk` to `contacts(id)` with
  `ON DELETE SET NULL` (deleting an address-book contact unlinks, it never
  destroys billing history), and defence-in-depth CHECKs on name/country/
  currency/terms that the store already enforces in Rust.
- **`platform/alo-store/src/billing_customers.rs`** — `NewCustomer` (the
  writable shape, with EU defaults: `EUR`, 30-day terms), `Customer` (the
  stored record), and the CRUD on `AccountStore`:
  `create_billing_customer`, `billing_customers(include_archived)`,
  `billing_customer`, `update_billing_customer`,
  `set_billing_customer_archived`. One `normalize()` runs for both create and
  update, so a field cannot be stored two ways depending on the door: name
  trimmed and bounded, country/currency uppercased (shape-checked, not
  list-checked — see the cut below), email shape-checked, VAT id compacted
  (whitespace/dots/hyphens stripped, uppercased) and left `None` for B2C.
- **No delete**: archiving is the only removal, per the design note — an
  issued invoice must always be able to name its customer. Re-archiving keeps
  the original `archived_at`; archived rows sort after active ones.

Two decisions worth recording:

- **`StoreError::Validation(String)` added** (`error.rs`). The billing error
  map needs `422 fixable input` distinct from `409 conflicts with state`, and
  the store had only `Conflict` for both. Every existing `StoreError` match in
  `alo-jmap` has a catch-all arm, so this is additive; the arm that maps it to
  `422` lands with the routes in B1.05.
- **`archived_at TIMESTAMPTZ` rather than the boolean `archived` flag** the
  design note sketched: same semantics, and it answers "since when" for free.
  Folded into the note at the B1.27 as-built pass.

Verified: `SQLX_OFFLINE=true cargo clippy --workspace --all-targets` clean
(no warnings), `cargo test -p alo-store` fully green against the local
Postgres (`alo-pg`), including the new suites — 9 pure unit tests over the
normalisation rules and `tests/billing_customers_tenancy.rs`:
`billing_customers_round_trip_and_never_cross_tenant` (the mandatory
wrong-tenant proof: tenant B gets `None`/empty/`NotFound` on read, list,
update, and archive, A's row is unchanged after every attempt, an id that
never existed gets the *same* answer as another tenant's id, and
`delete_tenant` purges the rows — checked by reading the table directly) and
`a_customer_can_only_link_a_contact_of_its_own_tenant`. Schema confirmed in
the live dev database with `\d billing_customers`. No new routes, so no wire
verification applies to this item; nothing user-visible changed, so no
CHANGELOG line (the first one lands with B1.05's routes).

Cuts: country and currency are validated by **shape** (two/three ASCII
letters, uppercased), not against a list of assigned ISO codes — a stale list
blocks a real customer, and the codes that actually matter are pinned by the
VAT rules (B1.03) and the FX table (B1.21). Recorded here rather than
silently.

Next item: B1.03 (VAT-id format validation wired into customer create/update).

## 2026-08-06 — B1.03 VAT-id validation

Shipped `platform/alo-store/src/vat_id.rs`, a pure module (no database, no
network) that validates and canonicalises a VAT identification number for a
customer's country, wired into `create_billing_customer` and
`update_billing_customer` through the one `normalize()` both already share.

- **Shape rules for all EU-27**, keyed on the VAT prefix rather than the
  country code (Greece is `GR` as a country and `EL` as a prefix, and that
  mapping is handled).
- **Check digits for 14 member states** — AT, BE, DE, DK, FI, FR, IT, LU,
  NL, PL, PT, SE, SI, SK — each one pinned in the tests by a real,
  independently-known VAT id plus a mistyped twin that must fail, so a
  transcription slip in an algorithm fails the suite rather than a customer.
- **The stored form is canonical**: uppercase, separators removed, and always
  carrying its two-letter prefix (`DE 811.907-980`, `811907980` and
  `de811907980` all store as `DE811907980`) — the form EN 16931 and every
  e-invoicing schema want. This is a change from B1.02, which stored the
  compacted string as typed; nothing is deployed, so no data migration is
  involved.
- **A foreign registration is kept as written** when it is valid for the
  country it names (a German customer really can invoice under a French
  number), and when an id names a country of its own but is broken, the error
  reports *that* country's rule rather than the customer's.
- **Empty stays empty**: no VAT id, or one that is only separators, is a B2C
  customer, never an error.
- Errors carry the rule and the country prefix but **never the id itself** —
  customer data does not travel into logs (law 1), asserted by a test.

Verified: `SQLX_OFFLINE=true cargo clippy --workspace --all-targets` clean
(zero warnings), `cargo test -p alo-store` green against local Postgres —
104 unit tests including 12 new ones over the VAT rules (every member state
accepts its own real id; malformed and mistyped ids refused; prefix optional
on input, always present on output; separators/case are presentation; the
B2C blank; foreign registrations; the French key that looks like a country
code; charset/length before everything; errors never echo the id) — plus the
`billing_customers_tenancy` integration suite, whose wrong-tenant proof still
passes and which now also refuses a right-shape/wrong-check-digit German id
and a Dutch-prefixed nine-digit one on both the create and the update path.
`rustfmt --check` clean on both touched files (formatting stayed inside this
item's lines — the divergence noted in the inbox above is untouched). No new
routes, so no wire verification applies; nothing user-visible yet, so no
CHANGELOG line (the first one lands with B1.05's routes).

Cuts, and the reasoning behind them — **flagged for human review, since VAT
ids are compliance-adjacent**:

- **13 member states pass on shape alone** (BG, CY, CZ, EE, EL, ES, HR, HU,
  IE, LT, LV, MT, RO). Their check algorithms are either unpublished, or
  published in several mutually-inconsistent variants that I could not pin
  to a known-good sample offline. A wrong checksum **rejects a real customer
  and makes them un-invoiceable**; a missing one only means a typo is caught
  later. Silence was chosen over guessing, deliberately.
- **NL post-2020 sole-trader ids** (letters in the first block, not
  BSN-derived) pass on shape alone for the same reason; the classic
  all-digit "elfproef" is enforced.
- **FR alphanumeric keys** (issued since 2014) pass on shape alone; the
  numeric-key rule `(12 + 3 × (SIREN mod 97)) mod 97` is enforced.
- Existence is **not** checked: a live VIES lookup is a network call, which
  is out of scope here and something the loop must never make. If we want
  "this number is really registered", it becomes its own queue item with an
  explicit user-triggered lookup and a cached result.

Next item: B1.04 (migration + store for `billing_products`).
