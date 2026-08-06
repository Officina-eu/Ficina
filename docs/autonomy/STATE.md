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

## 2026-08-06 — B1.04 billing products (migration + store)

Shipped the tenant's price list, plus the shared field rules the rest of the
wave will sit on:

- **Migration `0101_billing_products.sql`** — tenant-scoped,
  `PRIMARY KEY (tenant_id, id)`, `REFERENCES tenants(id) ON DELETE CASCADE`,
  `unit_price_cents BIGINT` and `vat_rate_bp INTEGER` (no floating-point
  column exists anywhere in billing), an `archived_at` timestamp rather than
  a boolean `active` — the same shape as `billing_customers`, so the pickers
  and the `/archive` route behave identically across the module — and
  defence-in-depth CHECKs on name/price/rate that the store already enforces
  in Rust. Index `(tenant_id, lower(name))` for the list surface.
- **`platform/alo-store/src/billing_products.rs`** — `NewProduct` (the
  writable shape) and `Product` (the stored record), with the CRUD on
  `AccountStore`: `create_billing_product`, `billing_products(include_
  archived)`, `billing_product`, `update_billing_product`,
  `set_billing_product_archived`. One `normalize()` runs for both create and
  update. Archiving stays a separate call from editing, so a price change can
  never drop an item out of the pickers by accident, and it is idempotent
  (re-archiving keeps the original time).
- **`platform/alo-store/src/billing_field.rs`** (new, small) — the primitive
  rules every billing record shares: `bounded`, `required`, `vat_rate_bp`,
  `unit_price_cents`, with `VAT_RATE_MAX_BP` and `UNIT_PRICE_MAX_CENTS`.
  `billing_customers.rs` was moved onto it in the same commit (its private
  `bounded`/`validate_name` are gone), so customers, products, and the
  invoice/quote lines coming in B1.06 answer a caller with one wording per
  rule instead of three.

Two decisions worth naming, both recorded in the module docs:

- **The price ceiling is arithmetic, not taste.** `UNIT_PRICE_MAX_CENTS` is
  10^9 (€10 000 000.00 per unit) because B1.06 computes line net as
  `qty_milli × unit_price_cents / 1000`; that cap keeps the product inside
  `i64` for any quantity the line model can hold, so no document total can
  wrap into a wrong number. A test asserts the multiplication at both
  ceilings is still an `i64`.
- **Negative prices are refused.** A discount is a negative quantity or a
  credit note (B1.09) — both auditable — whereas a negative unit price hides
  a refund inside an ordinary line.

Verified: `SQLX_OFFLINE=true cargo clippy -p alo-store --all-targets` clean
(zero warnings), `cargo test -p alo-store` green against local Postgres —
115 unit tests, 11 of them new (the shared field rules, and the product
normalisation: trimming, the required name at and past its bound, the
optional unit, the price floor/ceiling, the real European VAT spread plus the
exempt zero) — and the new `billing_products_tenancy` integration suite,
which proves the CRUD arc and, on every path (read, list, update, archive),
that another tenant gets the clean `NotFound`/empty and that a ghost id is
indistinguishable from another tenant's id. It also pins that cents survive
the round trip exactly, that active rows sort before archived ones, that a
rejected write leaves the record untouched, and that deleting the tenant
purges the rows — read back with a direct `count(*)`, not through the store's
own tenant predicate. `\d billing_products` inspected on the live local
database: the three CHECKs and the cascade are on the table as written.
`rustfmt --edition 2024 --check` clean on all six touched files.

No new routes (B1.05), so no wire verification applies; nothing user-visible
yet, so still no CHANGELOG line — the first one lands with B1.05's routes.

Cuts and flags:

- **No `currency` column on a product.** The design note's model doesn't
  carry one, and a price list is quoted in the tenant's own currency; the
  document carries the currency it was raised in and B1.21 adds the FX
  snapshot. Noted in `docs/design/billing.md` so the ambiguity isn't left to
  be rediscovered. Additive to add later if a tenant really keeps two lists.
- **`unit` is free text**, bounded at 32 characters. EN 16931 wants a
  UN/ECE Recommendation 20 unit code on the line instead — that mapping is
  the e-invoice writer's job (B1.22) and is flagged there rather than guessed
  at here, in line with the loop's rule on compliance items.
- **No SKU/barcode/purchase price** — those are explicitly B5.02's catalogue
  upgrade, not this item.

Next item: B1.05 (HTTP `/billing/customers` + `/billing/products` routes).

## 2026-08-06 — B1.05 billing customers + products HTTP routes

The first `/billing/*` routes. Three new files in `products/mail/alo-jmap/src`,
registered in `server.rs` between the Spaces block and Drive:

- **`billing.rs`** — the shared edge every future `/billing/*` module reuses:
  the store-error → HTTP map the design note publishes (`NotFound` → 404,
  `Validation` → 422 carrying the rule, `Conflict` → 409, everything else an
  opaque 500), body parsing that answers `400 malformed request body` without
  ever echoing the request, the RFC 3339 stamp, a forgiving boolean query flag,
  and `absent_or_null` — the `Option<Option<T>>` deserializer that keeps
  "absent" and "explicit null" apart so a `PATCH` can actually clear a field.
- **`billing_customers.rs`**, **`billing_products.rs`** — `GET`/`POST` on the
  collection, `GET`/`PATCH` on the item, `POST …/archive`.

Three conventions, chosen once and documented in the module headers so B1.10's
invoices inherit them rather than re-deciding:

- **No validation lives at the route layer.** Every rule stays in the store,
  because the billing agent (B1.25) calls the store directly and must not get a
  second, weaker definition of valid.
- **Every write answers with the stored record**, read back after the write. The
  caller sees the canonical form (`de` → `DE`, `" de 811.907-980 "` →
  `DE811907980`) rather than what it sent — and a misspelled field name is
  visibly absent from the answer instead of silently dropped, which is why
  unknown fields are ignored rather than rejected (the surface has to stay
  additively evolvable).
- **`PATCH` is a merge onto the stored record, then a full replace.** One
  `apply()` serves both create (merged onto the type's defaults) and edit, so a
  field cannot mean one thing on create and another on edit. Archiving is its
  own `POST`, never a field on the `PATCH`.

Verified. `SQLX_OFFLINE=true cargo clippy -p alo-jmap -p alo-store
--all-targets` clean (zero warnings); `cargo test -p alo-jmap` fully green —
every pre-existing suite plus the new `tests/billing_http.rs` (12 tests through
the real router over local Postgres) and 14 new unit tests in the three
modules. The **wrong-tenant test** is the centre of that suite: tenant A gets
404 from `GET`/`PATCH`/`archive` on tenant B's customer *and* product ids, A's
lists never mention them (`includeArchived` included), the refusals never echo
the record they refused, B's rows are unchanged afterwards, and the same denial
is then re-proved through the store handle directly so it does not rest only on
the routes. Alongside it: the 401 guard on every verb, the 422s naming their
rule, `null`/`""` clearing a nullable field, an empty `PATCH` changing nothing,
zero being a stated value rather than an absent one, idempotent re-archiving,
and a `400` for `19.99` in a cents field that never quotes the body back.

Wire-verified with real curl against the local debug `alo-jmap` on
`127.0.0.1:8080` over docker `alo-pg` (fresh tenant `wireb105`), full
transcript:

```
GET  /billing/customers            (no token)                        -> 401
POST /billing/products             (no token)                        -> 401
POST /billing/customers            vatId DE811907981                 -> 422  "the check digit of this DE VAT id does not match; check for a typo"
POST /billing/customers            name "   "                        -> 422  "name must not be empty"
POST /billing/products             unitPriceCents 19.99              -> 400  "malformed request body"
POST /billing/customers            " Acme GmbH ", de, " de 811.907-980 " -> 200  name "Acme GmbH", country "DE", currency "EUR", vatId "DE811907980"
GET  /billing/customers                                              -> 200  1 record
GET  /billing/customers/{id}                                         -> 200
PATCH/billing/customers/{id}       {city, paymentTermsDays}          -> 200  city+terms changed, name/vatId/postalCode intact
PATCH/billing/customers/{id}       {"vatId":null}                    -> 200  vatId null
POST /billing/customers/{id}/archive {"archived":true}               -> 200  archived, archivedAt set
GET  /billing/customers                                              -> 200  []
GET  /billing/customers?includeArchived=1                            -> 200  1 record
POST /billing/customers/{id}/archive {"archived":false}              -> 200  restored
GET  /billing/customers/no-such-id                                   -> 404
PATCH/billing/customers/no-such-id                                   -> 404
POST /billing/products             " Consulting ", hour, 12500, 2100 -> 200
GET  /billing/products                                               -> 200  1 record
PATCH/billing/products/{id}        {"unitPriceCents":13000}          -> 200  price changed, name/rate intact
POST /billing/products/{id}/archive                                  -> 200
GET  /billing/products                                               -> 200  []
```

Real rows read back with `psql` afterwards: one customer row for that tenant
with `vat_id` NULL (the clearing `PATCH` really landed), `city` Hamburg,
`payment_terms_days` 30; one product row with `unit_price_cents` 13000 of
`pg_typeof` **bigint** and `archived_at` set.

Cuts and flags:

- **HUMAN ACTION — `/billing` is a new top-level route prefix.** The production
  Caddyfile must add it at the next deploy or every billing route returns the
  SPA. The loop does not touch `deploy/`. (Flagged again at B1.27.)
- **A create answers `200`, not `201`.** Every other action route in `alo-jmap`
  answers `200` with the resource; one route inventing `201` is a wart a client
  has to special-case. Revisit for the whole surface at once, never per module.
- **No `If-Match`/`ETag`, so `PATCH` is last-writer-wins.** Two people editing
  different fields of one customer at the same instant lose one edit. Acceptable
  for a customer record; documents that carry money get concurrency control in
  B1.07/B1.08, where it is load-bearing.
- **A foreign VAT registration stays accepted.** A `DE`-prefixed valid id on an
  `NL`-addressed customer is stored as written — B1.03's documented rule, since
  a Dutch company registered for VAT in Germany really does invoice under a `DE`
  number. Pinned by a route test so a later reading cannot quietly tighten it.
  The country-decides rule still applies to unprefixed ids.
- **No web UI** — B1.13 owns that; nothing in `web/` was touched.

Next item: B1.06 (`billing_invoices` + `billing_invoice_lines` migration, store,
and the pure totals function with property tests).

## 2026-08-06 — B1.06 invoices, lines, and the totals arithmetic

The document itself, and the one piece of arithmetic every later item in the
wave depends on. Four new files plus a migration:

- **Migration `0102_billing_invoices.sql`** — `billing_invoices` and
  `billing_invoice_lines`. The lifecycle is in the constraints, not only in
  Rust: `status IN (draft|issued|paid|void)`, `(status = 'draft') =
  (number IS NULL)` and the same for the dates, so a **numbered draft** and an
  **issued document without a number** are both states the database refuses;
  `UNIQUE (tenant_id, number)`; a composite FK `(tenant_id, customer_id)` →
  `billing_customers`, so a cross-tenant customer link is impossible even if a
  `WHERE` clause were ever wrong; a nullable self-FK for the credit note
  (B1.09) with `is_credit_note = (credits_invoice_id IS NOT NULL)`. Lines
  cascade from their invoice and reach their tenant only through it.
- **`billing_totals.rs`** (pure) — `LineFigures`, `VatSubtotal`, `Totals`,
  `line_net_cents`, `totals`. No database, no clock, no tenant: the single
  place money is computed, so invoices, quotes, the PDF and the e-invoice XML
  cannot drift apart.
- **`billing_line.rs`** — the line shape and its rules, shared with quotes at
  B1.11: description/unit bounds, quantity in milli-units (negative allowed —
  that is a discount), `MAX_LINES = 500`, and a rejection message that names
  *which* line failed (1-based, as the user sees it) without ever echoing the
  line's text.
- **`billing_invoices.rs`** — `InvoiceStatus`, `NewInvoice`, `Invoice`,
  `InvoiceSummary`, `InvoiceDocument`, and the store: create a draft, read one
  document with lines+totals, list with a status filter, replace the header,
  replace the whole line set in one transaction, and `billing_line_totals` —
  the same arithmetic *before* writing, so the B1.14 draft editor shows live
  totals from the server instead of computing money in the browser.
- **`billing_field.rs`** gained the currency and payment-terms rules (moved out
  of `billing_customers.rs`, which now uses them): invoices need the same two
  rules, and one wording per rule across the module is the point of that file.

Decisions worth recording:

- **Rounding is half away from zero, not half up** — and this is a compliance
  decision, so it is flagged rather than buried. Rounding happens once at the
  VAT-rate subtotal (EN 16931 BR-CO-17), never per line. The two conventions
  agree on positive amounts; away-from-zero is what makes a credit note the
  exact mirror of its original (`totals(−lines) == −totals(lines)`, a
  property test), whereas half-up leaves a one-cent residue whenever a credit
  rounds at a half — a ledger that does not sum to zero. Recorded in
  `docs/design/billing.md`, which had said "half-up" while leaving negatives
  unconsidered.
- **Totals are never stored.** They are derived from the lines on every read,
  so no client can influence what a document is worth and no column can drift
  from the lines that justify it. The list surface fetches every listed
  document's lines in one further statement, not one per document.
- **Lines are written as a whole set**, in one transaction, with the invoice
  row locked `FOR UPDATE`: two editors saving at once serialise instead of
  interleaving line sets. Every line is validated before anything is written,
  so a bad line at the end cannot leave a half-replaced document. The
  draft-only guard (B1.07) lands on that same lock.
- **All arithmetic is `i128` internally, narrowed with saturation.** The
  validated bounds (|qty| ≤ 10^9 milli, price ≤ 10^9 cents, ≤ 500 lines) put a
  document's gross four orders of magnitude below `i64::MAX`; the saturation is
  the guarantee that the pure function is total for *any* caller — no wrap into
  a plausible wrong number, no panic.
- **A new document cannot be raised for an archived customer** (typed 422).
  Archiving means "we no longer bill them"; existing documents still name them,
  which is what archiving is for.

Verified: `SQLX_OFFLINE=true cargo clippy -p alo-store -p alo-jmap
--all-targets` clean (zero warnings); `cargo test -p alo-store` fully green
against local Postgres — 142 unit tests (27 new: 15 over the totals module, 9
over the line rules, 3 over the status enum) and every integration suite,
including the new `tests/billing_invoices_tenancy.rs` (3 tests).

The **property tests** the item asked for run 19 000 generated documents
through a deterministic seeded generator (xorshift64*, no new dependency, so a
failure always reproduces): line sums always reconcile to the returned totals
and to the per-rate subtotals; each rate appears exactly once, ascending, and
the rate set is exactly the document's; every subtotal's VAT is the rate
applied once to that subtotal's net, recomputed independently of the
implementation; `gross == net + vat` always; negation is an exact mirror; line
order never changes an answer; a zero rate never produces VAT. Plus the
boundary cases: a 500-line document at every validated ceiling stays an order
of magnitude inside `i64`, and absurd input saturates rather than wrapping.

The **wrong-tenant proof** covers every path: tenant B gets `None`/empty from
read and list and `NotFound` from header update and line replacement on A's
document; A's document is unchanged after each attempt; a ghost id gets the
same answer as another tenant's id (no existence oracle); and the customer
link cannot cross — raising or re-pointing a document at another tenant's
*real* customer id is `NotFound`, not a cross-tenant link. Alongside it: a
second suite proving one document's line replacement never touches another's,
and a third round-tripping a full 500-line document exactly (milli-units and
cents intact, totals hand-checked).

Wire-checked on the live local database with `psql`: `\d billing_invoices` and
`\d billing_invoice_lines` show every constraint as written, and four direct
SQL probes proved the claims the Rust tests cannot reach yet — a numbered
draft is refused, an issued document without dates is refused, two documents
of one tenant cannot share a number, and an invoice for **another tenant's**
customer id is refused by the foreign key itself. A fifth probe confirmed that
deleting a tenant still purges cleanly when a credit note references another
invoice (the self-FK does not block the cascade), which is what B1.09 will
build on.

Cuts and flags:

- **No FX column yet.** The design note lists a stored FX-rate snapshot on the
  invoice; it belongs to B1.21 and arrives as an additive `ALTER TABLE` then,
  rather than sitting unvalidated in the schema for fifteen items.
- **No delete, and no draft-only guard.** Deleting an abandoned draft and
  refusing edits to a non-draft are B1.07's item; nothing in B1.06 can move a
  document off `draft`, so the guard would be untestable code today. The lock
  it will sit on is already in place.
- **`is_credit_note` / `credits_invoice_id` are written but never set** —
  B1.09 sets them. They are in the table now because the numbering and status
  constraints are stated in terms of them.
- **No routes** (B1.10), so no curl transcript applies to this item; nothing
  user-visible changed, so still no CHANGELOG line.

Next item: B1.07 (draft-invoice lifecycle — edits only while draft, typed
error on a non-draft).

---

## 2026-08-06 — B1.07 the draft-invoice lifecycle

A billing document is editable exactly while it is a draft, and from this
item the store enforces that rather than describing it. `InvoiceStatus::
ensure_editable` is the single rule — a draft may be changed, an `issued`,
`paid` or `void` document may not — and all three write paths run it:
`update_billing_invoice`, `set_billing_invoice_lines`, and the new
`delete_billing_invoice`. The refusal is a typed `StoreError::Conflict` whose
message names the status that refused (`409` at the route edge per the design
note's error map), never a silent no-op.

The guard sits **under the row lock, inside the writing transaction**. Every
write now takes `SELECT status … FOR UPDATE` and re-reads the state before it
touches anything, so a save composed against a draft that arrives while an
issue is in flight waits for that issue and is then refused, instead of
landing new lines on a document that has just been numbered and frozen. That
is the whole reason the check is not a cheap pre-read: B1.08's issuing
transaction will hold exactly this lock. `update_billing_invoice` also does a
cheap unlocked pre-check first, purely to fix the **error precedence** — a
frozen document is told it is frozen rather than being handed a complaint
about a field it was never going to accept.

Deletion is draft-only and complete: a draft never consumed a number, so
abandoning it leaves no hole in the gapless sequence and no record anyone is
entitled to; the lines go with it by cascade. An issued document is voided
(B1.08+), keeping its number and staying readable. Deleting a document does
not touch the customer it named.

Verified: `SQLX_OFFLINE=true cargo clippy -p alo-store -p alo-jmap
--all-targets` clean (zero warnings); `cargo test -p alo-store` green — 144
unit tests (2 new: the editable rule over all four statuses, and the proof
that a corrupt stored status is a decode failure rather than a guess that
would make a frozen document editable) plus every integration suite, and
`cargo test -p alo-jmap` green (88 unit + all suites), since the store API
changed underneath it.

The new `tests/billing_invoice_lifecycle.rs` (5 tests) is the item's proof.
Issuing does not exist yet, so the issue marker is planted with raw SQL —
`status`, `number`, `issue_date` and `due_date` set together, which is exactly
the state the table's CHECK constraints define as *not a draft*. That is
deliberate: the guard must hold against the **stored** state of the row, not
against whatever the Rust API happened to write. The tests prove, for each of
`issued`/`paid`/`void`: header update, line replacement (including emptying
it) and deletion are all refused with a `Conflict` naming the status; a bad
payload against a frozen document still gets the `Conflict`, not a validation
complaint; and afterwards the document is unchanged down to `updated_at`, its
number, its line rows read straight from the table, and its totals. A
companion test shows the same calls all succeeding while the document is a
draft — and a bad line there still being judged on its content — so the guard
is not simply refusing everything.

The race is proven, not argued: a transaction issues the document and holds
its lock uncommitted; a `set_billing_invoice_lines` fired into that window is
observed still waiting after 250 ms (it did not read a status the issue was
about to change), and once the issue commits it returns `Conflict` and wrote
nothing.

Wrong-tenant proof: tenant B gets `NotFound` — never `Conflict` — for delete,
header update and line replacement on A's document, whether A's document is
an editable draft or a frozen issued one. `Conflict` there would have
confirmed both that the id exists and what state it is in; a ghost id gets the
identical answer, and B's own draft of the same shape deletes cleanly, so the
denial is about ownership and not about the operation being unavailable. A's
documents and their lines are intact afterwards.

Cuts and flags:

- **No route yet** (B1.10), so no curl transcript applies and nothing
  user-visible changed — still no CHANGELOG line. `docs/design/billing.md` now
  lists `DELETE /billing/invoices/{id}` as draft-only in the surface table,
  adds the two `409` rows to the error map, and records the as-built rule.
- **Voiding is not implemented** — it belongs with issuing (B1.08). Today
  nothing in the Rust API can move a document off `draft`, which is why the
  tests plant the marker in SQL.
- **A draft referenced by a credit note cannot arise**, so delete needs no
  guard against the self-FK: only an *issued* document can be credited
  (B1.09), and an issued document cannot be deleted.
- `platform/alo-store/src/lib.rs` was left untouched on purpose: running
  `cargo fmt` re-wrapped a pre-existing over-long `use` line there, and that
  file is shared with the sites track (additive lines only), so the churn was
  reverted rather than pushed into a rebase conflict.

Next item: B1.08 (the issue flow — per-tenant gapless sequence, `INV-YYYY-
NNNNN`, row-locked in the issuing transaction, with the 100-iteration
concurrency test).
