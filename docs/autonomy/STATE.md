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

---

## 2026-08-06 — B1.08 the issue flow and legally gapless numbering

A draft becomes a legal document. Shipped:

- **Migration `0103_billing_sequences.sql`** — `(tenant_id, kind, year) →
  next_value`, the counter behind the numbering. `kind` is **shape**-checked
  (`^[a-z_]{1,32}$`) rather than list-checked, so quotes (B1.11) drawing their
  own series is a new row and never a schema change; `year` is bounded to
  2000–9999 and `next_value` to ≥ 2, which is definitionally true once a row
  exists (the row is created at 2 by the draw that takes 1). Cascades with the
  tenant.
- **`platform/alo-store/src/billing_sequence.rs`** (new) — the series and the
  printed form of a number, in their own file because they change for a
  different reason than the invoice does: credit notes (B1.09) and quotes
  (B1.11) draw from here too. `document_number()` prints
  `INV-YYYY-NNNNN`; `draw_next()` is one upsert that both creates the series
  on first use and advances it, holding the counter's row lock until the
  issuing transaction ends.
- **`AccountStore::issue_billing_invoice`** — one transaction: lock the
  document, refuse anything but a draft (`Conflict`), refuse an empty one
  (`Validation`), read the database's own `CURRENT_DATE`, draw the number,
  write number + issue date + due date + `issued`, commit, and return the
  frozen document with its totals.
- **`AccountStore::void_billing_invoice`** — the exit B1.07 deferred to this
  item: `issued → void`, keeping the number, the dates and the lines.
- `InvoiceStatus::ensure_issuable` / `ensure_voidable` alongside the existing
  `ensure_editable`, so each refusal names both the transition and the status
  that refused it instead of one generic message.

Three decisions, all recorded as as-built in `docs/design/billing.md`:

- **The issue date is the database's today, not a caller's date.** A series
  whose numbers ascend while their dates do not is not gapless in any sense a
  tax authority accepts. Flagged below.
- **An invoice with no lines cannot be issued** — `Validation` (422), not
  `Conflict`, because the caller fixes it by adding a line. It would spend a
  number of a legally unbroken series on a document that says nothing.
- **Voiding is `issued`-only.** A draft is deleted (it took no number), a paid
  document is corrected with a credit note (B1.09), and a void one is already
  void. The design note now records that a document the customer already holds
  should be credited rather than voided — the store cannot tell the two cases
  apart, so it allows the transition and says so rather than guessing.
- **Lock order is document, then counter, on every path**, so concurrent
  issues queue instead of deadlocking.

Verified: `SQLX_OFFLINE=true cargo clippy -p alo-store -p alo-jmap
--all-targets` clean (zero warnings); `cargo test -p alo-store` fully green
against local Postgres — 148 unit tests (4 new over the number format: the
padding, the sixth digit past 99 999 rather than a wrapped duplicate, the
lexicographic sort that padding buys, the four-digit year) and every
integration suite; `cargo test -p alo-jmap` green as well, since the store API
moved underneath it. `rustfmt --edition 2024 --check` clean on all three
touched/added Rust files (`lib.rs` was left alone but for its two additive
lines — the pre-existing divergence in the inbox above is untouched).

The item's gate is `tests/billing_invoice_issue.rs` (8 tests):

- **`a_hundred_parallel_issues_never_share_or_skip_a_number`** — 100 drafts,
  100 issues fired at once at one tenant's series, and the resulting numbers
  compared against the exact set `INV-YYYY-00001..00100`: sharing a number
  would be two legal documents with one number, skipping one would be a hole a
  tax inspection reads as a deleted invoice, and both fail this test. The
  counter is then read back (101) and the distinct numbers counted straight
  from the table. Green on three consecutive runs.
- **The test was proved non-vacuous by a negative control**: `draw_next` was
  temporarily replaced with the naive read-then-write (no lock, two
  statements), and the concurrency test failed immediately; the real upsert was
  then restored and re-run. A concurrency test that has never been seen to fail
  is not evidence.
- **`a_rolled_back_draw_gives_its_number_back`** — the property `nextval()`
  cannot provide, and the whole reason the counter is a row: the same upsert is
  run in a transaction that then rolls back, the counter is proven gone, and a
  real invoice then takes the number the failed attempt had drawn.
- `an_invoice_with_no_lines_never_consumes_a_number` — the refusal is a
  `Validation`, the counter row is never even created, the next real document
  is still number 1, and the same invoice issues cleanly once it has a line.
- `each_tenant_and_each_year_counts_alone` — two tenants both issue number 1
  (correct: the series is per tenant), and a seeded previous-year row at 900 is
  neither read nor moved by this year's issue.
- **The wrong-tenant proof**, `another_tenant_can_neither_issue_nor_void_nor_
  learn_the_state` — tenant B gets `NotFound`, never `Conflict`, from issue and
  void on both a draft and an issued document of A's (a `Conflict` would have
  confirmed the id exists *and* what state it is in); a ghost id gets the
  identical answer; A's documents are unchanged down to `updated_at`; **A's
  counter is unmoved by B's attempts**; and B can issue their own document of
  the same shape, so the denial is about ownership rather than the operation.
- `issuing_numbers_dates_and_freezes_the_document` and
  `voiding_keeps_the_number_…` pin the rest: dates from the database's clock,
  the due date at issue + terms, the document unchanged by issuing (same
  lines, same totals), every write path and a second issue refused afterwards,
  a voided document keeping number/dates/lines and not releasing its number
  back to the series.
- `a_save_that_races_the_real_issue_loses_cleanly` re-proves B1.07's race
  against the **real** issuing transaction rather than a planted marker:
  whichever won the lock, the stored document is coherent and the loser wrote
  nothing.

Schema confirmed on the live local database (`\d billing_sequences`): the
three CHECKs, the composite primary key and the tenant cascade are on the
table as written.

Cuts and flags:

- **FLAGGED FOR HUMAN REVIEW (compliance-adjacent): no backdated issuing.**
  Issuing stamps the database's today. Bookkeepers do sometimes need to issue
  "as of" an earlier day (a month-end run done on the 3rd), and the strict
  reading of the gapless-numbering rules is what is implemented here: numbers
  and dates must ascend together. Offering backdating needs a rule that keeps
  those two orders consistent (a cut-off window, or a per-year series that
  refuses a date earlier than the last issued one) — that is its own queue
  item, not a quiet parameter on this one.
- **A voided document carries no reason.** A reason column is a real
  requirement in some jurisdictions' audit trails, but it belongs with the
  cross-cutting audit log (B2.13) rather than as a lone free-text column here.
- **No routes** (B1.10 owns `/billing/invoices`, including `POST …/issue` and
  the `POST …/void` now added to the design note's surface table), so no curl
  transcript applies to this item, and nothing user-visible changed — still no
  CHANGELOG line. The first one lands with B1.10.
- **Contention is a plain row lock, with no retry or timeout tuning.** At SME
  volume the issuing transaction is sub-millisecond; the design note's `503`
  row for contention beyond a retry stays a route-layer concern for B1.10.

Next item: B1.09 (credit notes — a negative document referencing an issued
original, drawing from the same series, whose ledger with the original sums to
zero).

---

## 2026-08-06 — B1.09 credit notes and the ledger that closes

The correction a customer's copy can be reconciled against. Shipped:

- **Migration `0104_billing_credit_notes.sql`** — expand-only, and no new
  table: `is_credit_note` and `credits_invoice_id` have been on
  `billing_invoices` since `0102`, together with the CHECK tying them to each
  other and the composite FK keeping the credited document inside the tenant.
  This migration adds the two things the *relation* needs: a CHECK that a
  document cannot credit itself (a one-row cycle every walk of the credit chain
  would have to defend against) and a partial index on
  `(tenant_id, credits_invoice_id)` for the read below.
- **`AccountStore::create_billing_credit_note(original)`** — one transaction
  under the original's row lock: refuse what cannot be credited, then insert a
  **draft** carrying the original's customer, currency, terms and customer
  reference, and copy every line in print order with its quantity negated.
- **`AccountStore::billing_credit_notes(original)`** — the read side: what
  credits this document, with each one's computed totals. Without it the
  relation would be write-only, and the ledger of a corrected invoice
  unanswerable.
- **`InvoiceStatus::ensure_creditable`** alongside the existing
  editable/issuable/voidable guards.
- `lock_invoice_status` became **`lock_invoice`**, returning the handful of
  stored facts a write decides against (status, credit-note flag, customer,
  currency, terms, reference) instead of only the status. Two of the new
  decisions are about what a document *is*, not where it is, and they must be
  read under the same lock as the status. The issue path lost its extra
  `SELECT` for the terms as a result.

Decisions, all recorded as as-built in `docs/design/billing.md`:

- **A credit note is an invoice in the same table, on the same series** — not a
  second document type with a `CRN-` prefix of its own. An unbroken ledger is
  one series; two prefixes sharing one counter would print as two series each
  full of holes. Issuing a credit note therefore goes through the ordinary
  `issue_billing_invoice`, with the same freezing rules.
- **It is created as a draft, mirroring the whole original.** The mirror is the
  starting position, not the finished document: a **partial** credit is made by
  editing its lines before issuing. That is why no line-sign rule is imposed.
- **The customer and currency are pinned** to the original's while editing
  (`Validation`, 422). A credit billed to somebody else, or in another
  currency, reverses nothing. Everything else — terms, reference, note, lines —
  stays freely editable.
- **The note is *not* copied.** The original's "payable within 14 days" says
  the opposite of the truth on a credit note. The link to the original is
  structural (`credits_invoice_id`), so B1.16's print view can render
  "credit note for INV-…" as an i18n string rather than the store inventing
  English prose.
- **An archived customer can still be credited.** The customer is copied, not
  re-resolved through `normalize_invoice`, so archiving cannot trap a wrong
  invoice in the ledger forever. Raising a *new* invoice for them stays refused
  — that guard is about new business.
- **`issued` and `paid` are creditable; `draft` and `void` are not.** The queue
  said "original must be issued"; a paid invoice was issued, and it is the case
  credit notes exist for (the design note already said a paid document is
  corrected, never voided). A draft is deleted instead, and a void document has
  been cancelled in full already.
- **The credit-note refusal outranks the status refusal.** Crediting a credit
  note is refused for what the document *is*, so the answer does not change
  when the same document is later issued — a UI must simply never offer the
  action there. (The first cut had the checks the other way round and the test
  caught it: a fresh credit note was refused for being a draft.)

Verified: `SQLX_OFFLINE=true cargo clippy -p alo-store -p alo-jmap
--all-targets` clean (zero warnings); `cargo test -p alo-store` fully green
against local Postgres — 149 unit tests (1 new over `ensure_creditable`) and
every integration suite; `cargo test -p alo-jmap` green as well (88 unit + all
suites), since the store's locking read changed underneath it.
`rustfmt --edition 2024 --check` clean on both touched Rust files, and the
formatting run touched only lines this item added (the pre-existing divergence
in the inbox above is untouched).

The item's gate is `tests/billing_credit_notes.rs` (6 tests):

- **`an_issued_invoice_and_its_credit_note_sum_to_zero`** — the done-when. The
  fixture is deliberately awkward: three VAT rates plus a zero rate, a discount
  line with a negative quantity inside the *original*, a line whose net lands
  on a third of a cent (0.333 h × €99.99) and one that lands exactly on a half
  (0.5 × €11.11 → 555.5). Original and credit are added the way a ledger adds
  them — net, VAT, gross **and every row of the per-rate breakdown** — and every
  figure is 0, with no rate left over. It then checks the mirror line by line
  (same order, same description/unit/price/rate, negated quantity, its own row
  id), issues the credit note and asserts the numbers are `INV-YYYY-00001` and
  `INV-YYYY-00002` off **one** counter row (`next_value` 3), that issuing kept
  the link, that the frozen pair still sums to zero, and that the original can
  name what credits it.
- `a_draft_a_void_document_and_a_credit_note_are_all_refused` — each refusal
  typed and named, nothing written (no document, and the counter row never even
  created), and the credit-note refusal proven identical before and after the
  credit note is issued. A ghost id gets `NotFound`, not a state refusal.
- `a_paid_invoice_is_corrected_by_crediting_it_not_by_voiding_it` — the `paid`
  state is planted with SQL (payments are B1.19), so the guard is tested
  against the **stored** state rather than against what today's Rust API can
  produce: voiding it is refused, crediting it is not, crediting does not
  reopen it, and an archived customer is still creditable while a new invoice
  for them is still refused. Two credit notes against one original are both
  listed — partial corrections are several documents.
- `a_credit_note_draft_is_editable_but_stays_on_its_original` — the customer
  and currency moves refused typed with the document unchanged afterwards; then
  a real partial credit (keep one line of the mirror, drop the rest) that keeps
  the flag and the link, is worth less than the whole document, is still
  negative, and matches a hand-computed −€114.18 net / −€23.98 VAT.
- **`another_tenant_can_neither_credit_nor_discover_a_document`** — the
  mandatory wrong-tenant proof. B gets `NotFound` (never `Conflict`, which
  would confirm the id exists *and* its state) from crediting A's issued
  document, A's draft and A's own credit note; a ghost id gets the identical
  answer; `billing_credit_notes` on A's ids is empty for B and vice versa; A's
  counter is unmoved and no row anywhere outside A's tenant credits A's
  invoice (checked with a direct `count(*)`, not through the store's own tenant
  predicate); and B credits its own document of the same shape cleanly, so the
  denial is about ownership rather than the operation.
- `the_table_itself_refuses_an_impossible_credit_link` — the database, not the
  Rust: a self-credit, a cross-tenant credit link, and a `is_credit_note` flag
  without a named original are each rejected by direct SQL, and deleting a
  tenant still cascades cleanly with an issued credit chain in place.

Schema confirmed on the live local database (`\d billing_invoices`): the new
CHECK and the partial index are on the table as written.

Cuts and flags:

- **No over-credit guard.** Nothing stops a tenant raising credit notes worth
  more than the original. Refusing that needs the sum of *issued* credits
  against the gross, which is the same derived-state machinery B1.19 builds for
  paid/partially-paid; adding a second, weaker version of it here would be the
  thing that later disagrees. The read (`billing_credit_notes`) that such a
  guard needs is in place.
- **FLAGGED FOR HUMAN REVIEW (compliance-adjacent): the credit note's issue
  date is its own issue day, not the original's.** That follows B1.08's rule
  that numbers and dates ascend together, and it is the strict reading. Some
  jurisdictions expect a credit note to reference the original's date as well
  as its number; that is a *printing* concern (B1.16/B1.22 render both from the
  link), not a reason to backdate.
- **No routes** — `POST /billing/invoices/{id}/credit-note` is B1.10's, so no
  curl transcript applies to this item, and nothing user-visible changed:
  still no CHANGELOG line. The first one lands with B1.10.
- **No quote credit** — quotes do not exist yet (B1.11) and are not credited
  anyway.

Next item: B1.10 (the `/billing/invoices` HTTP routes — draft CRUD, issue,
void, credit-note, status-filtered list with overdue computed, and the
draft→issue→credit arc wire-verified with curl).

*Correction, same iteration:* commit `0364163` went out **without** the
`Co-Authored-By: Claude …` trailer every other loop commit carries — the
transparency record of which agent made the change (CLAUDE.md, "one agent per
working tree"). It was already pushed when this was noticed, and rewriting
pushed history is forbidden by the loop's safety rails, so the commit stands
and the gap is recorded here instead. The authorship itself is correct (the
repository owner, as configured).

---

## 2026-08-06 — B1.10 the invoice routes, and the arc on the wire

The door the web module (B1.13–B1.15) and the billing agent (B1.25) both come
through. Seven routes over the document that B1.06–B1.09 built, and the first
CHANGELOG line the wave has earned. Shipped:

- **`products/mail/alo-jmap/src/billing_invoices.rs`** — `GET/POST
  /billing/invoices`, `GET/PATCH/DELETE /billing/invoices/{id}`, and `POST
  …/issue`, `…/void`, `…/credit-note`, registered in `server.rs` under the
  existing `/billing` prefix.
- **`Invoice::is_overdue(today)`** in the store — the one definition of overdue
  (issued, and past the due date it was frozen with), so the list surface here,
  the overdue view (B1.19) and the dunning drafts (B1.26) cannot drift apart.
- **`billing::iso_date`** alongside `iso` — a billing date is a **day**, not an
  instant. Giving an issue date a time and a zone invites a client to shift it
  across midnight, and the date is the one thing on the document a tax
  authority reads together with the number.

Decisions, all recorded as as-built in `docs/design/billing.md` § Routes:

- **The header and the line set travel in one body.** `lines` is an ordinary
  field on both `POST` and `PATCH`, replacing the whole set in the order sent;
  absent, it leaves the stored lines alone. A draft editor saves the document
  it is looking at, not a patch stream — which is also the store's own model.
- **A body stating only `lines` does not touch the header.** Replaying the
  stored header would re-resolve the customer through `normalize_invoice`,
  which refuses an archived one; a draft whose customer was archived after it
  was raised would then be unable to have its lines edited at all — a dead end
  with no way out but deleting it. `states_header()` is that guard.
- **Money is only ever read.** Every response carries server-computed `totals`
  and a per-line `netCents`, and there is no writable total anywhere in the
  surface. No per-line VAT field either: VAT is rounded once per rate subtotal
  (B1.06), so a per-line column would not add up to the document's own and a
  client would render a document that disagrees with itself.
- **`overdue` is derived on read, and judged against the server's date.** Not a
  value a client may send: whether a document is late is a fact about the
  tenant's ledger, not about the reader's clock, and a browser with a wrong
  date must not be able to clear its own overdue list.
- **The `status` filter is strict — `422` on anything but the four states.**
  Deliberately unlike the forgiving boolean flags in `billing.rs`: a filter
  that silently widened to "everything" on a typo would show a bookkeeper
  drafts among their issued documents, which is the one list that must never be
  approximate.
- **Lines are validated before either write.** `billing_line_totals` (pure) runs
  first, so a typo in the last line cannot leave an empty draft behind on
  `POST`, nor a new header with the old lines on `PATCH`.
- **One check lives at the edge: `customerId` must be stated.** Which customer
  a document is raised for is not a field rule the store can own, and letting
  an absent id fall through would answer "no such customer" (`404`) to a
  request that never named one. Everything else is the store's.
- **Lifecycle transitions are their own `POST`s**, never fields on the `PATCH`,
  and `status`/`number`/`issueDate`/`dueDate` are not writable by any request.
- **`GET …/{id}` also answers `creditNotes`** — the ledger of a corrected
  invoice, drafts included, which the issued view (B1.15) needs.

Verified: `SQLX_OFFLINE=true cargo clippy -p alo-store -p alo-jmap
--all-targets` clean (zero warnings); `cargo test -p alo-store -p alo-jmap`
fully green against local Postgres (exit 0 across every suite);
`rustfmt --edition 2024 --check` clean on all four touched/added Rust files.

The item's gate is `products/mail/alo-jmap/tests/billing_invoice_http.rs`
(5 tests, all passing on the first run):

- **`the_draft_to_issue_to_credit_arc_runs_on_the_wire`** — the done-when,
  through the real router. A draft with three lines across two VAT rates,
  including a fractional quantity (1.5 h) and a price whose VAT lands on a half
  cent, comes back at net 26 747 / VAT 4 917 / gross 31 664 with the breakdown
  per rate in rate order — figures that only come out right if the server
  rounds once per rate. Then: a header-only `PATCH` leaves the lines and the
  totals alone; a lines-only `PATCH` replaces the set (including a negative
  discount line) and keeps the note; issuing assigns `INV-2026-00001`, today's
  date and today+14; all four write verbs then answer `409` naming the state
  and nothing moves; the credit note mirrors the lines with quantities negated
  and totals exactly negated; the original names it in `creditNotes`; issuing
  it draws `INV-2026-00002` from the **same** series; net, VAT and gross of the
  pair each sum to zero; the status filter partitions the three documents; a
  draft is deleted and a `404` afterwards, while the issued original is voided
  and keeps its number.
- `a_refused_request_writes_nothing_and_says_what_is_wrong` — a body naming no
  customer is a `422` about the field (never the `404` an unresolvable id
  gets); an unknown customer is a `404`; four kinds of bad line are each `422`
  with **no draft left behind**; `19.99` in a cents field is a `400` that never
  quotes the body; an empty document cannot be issued; a draft can be neither
  voided nor credited; a bad line in a `PATCH` leaves both the stored header
  and the stored lines as they were; three unrecognised status filters are
  `422` while a blank one is simply no filter.
- `every_route_needs_a_token_and_an_id_that_exists` — all eight route/verb
  pairs answer `401` without a token (the guard runs before anything is looked
  up, so an unauthenticated caller learns nothing about which ids exist) and
  `404` with one for an id that was never issued.
- `only_an_issued_document_past_its_date_is_flagged_overdue` — the past is
  planted with SQL, since the store refuses to backdate an issue (B1.08), so
  the flag is tested against the **stored** document: `true` on both the single
  read and the list, `false` again once voided without the due date moving, and
  `false` for a document due *today* (the customer has the whole day).
- **`another_tenants_document_is_invisible_and_untouchable_on_every_route`** —
  the mandatory wrong-tenant proof. A's lists never mention B's document on any
  filter and never leak its reference; all seven verbs on B's id answer A with
  `404` — never `409`, which would confirm the id exists *and* leak its state —
  and no refusal echoes what it refused; A cannot raise a document against B's
  customer either; B's document is unchanged afterwards and **B's next issue is
  still `…-00002`**, so A's refused attempts consumed none of B's numbers; and
  the denial is re-proved through A's store handle directly, past the routes.

Wire-verified with real curl against the local debug `alo-jmap` on
`127.0.0.1:8080` over docker `alo-pg` (fresh tenant `wireco`), full transcript:

```
GET   /billing/invoices                    (no token)                -> 401
POST  /billing/invoices                    (no token)                -> 401
POST  /billing/invoices/x/issue            (no token)                -> 401
POST  /billing/invoices/x/credit-note      (no token)                -> 401
GET   /billing/invoices/no-such-id                                   -> 404
POST  /billing/invoices                    {}                        -> 422  "customerId is required to raise a document"
GET   /billing/invoices?status=sent                                  -> 422  "status must be one of draft, issued, paid, void"
POST  /billing/customers                   Acme GmbH, DE             -> 200
POST  /billing/invoices                    3 lines, 2 rates          -> 200  draft, number null, EUR, terms 14, overdue false
                                                                            net 26747 / VAT 4917 / gross 31664; per-rate [700: 5000/350, 2100: 21747/4567]
                                                                            line nets [18750, 2997, 5000]
POST  /billing/invoices                    line description "  "     -> 422  "line 1: description must not be empty"
POST  /billing/invoices                    unitPriceCents 19.99      -> 400  "malformed request body"
GET   /billing/invoices                                              -> 200  1 (the refusals wrote nothing)
PATCH /billing/invoices/{id}               {note}                    -> 200  totals unchanged
PATCH /billing/invoices/{id}               {lines: 2, incl discount} -> 200  net 22500 / VAT 4725 / gross 27225; reference + note kept
POST  /billing/invoices/{id}/issue                                   -> 200  INV-2026-00001, issue 2026-08-06, due 2026-08-20, overdue false
PATCH /billing/invoices/{id}               {reference}               -> 409  "an invoice can only be changed while it is a draft; this one is issued"
DELETE/billing/invoices/{id}                                         -> 409  same
POST  /billing/invoices/{id}/issue         (again)                   -> 409  "an invoice can only be issued while it is a draft; …"
POST  /billing/invoices/{id}/credit-note                             -> 200  draft, creditNote true, credits {id}, qtys [-2000, 1000], gross -27225
GET   /billing/invoices/{id}                                         -> 200  creditNotes [(draft, -27225)]
POST  /billing/invoices/{cn}/issue                                   -> 200  INV-2026-00002, issued
GET   /billing/invoices/{id} + /{cn}                                 -> 200  27225 + -27225 = 0
POST  /billing/invoices                    {customerId} only         -> 200  an empty draft
POST  /billing/invoices/{empty}/issue                                -> 422  "an invoice with no lines cannot be issued; add a line first"
GET   /billing/invoices                                              -> 200  3
GET   /billing/invoices?status=draft                                 -> 200  [the empty draft]
GET   /billing/invoices?status=issued                                -> 200  [00002 -27225, 00001 27225], both overdue false
GET   /billing/invoices?status=paid                                  -> 200  []
GET   /billing/invoices?status=            (blank = no filter)       -> 200  3
DELETE/billing/invoices/{empty}                                      -> 200
GET   /billing/invoices/{empty}                                      -> 404
POST  /billing/invoices/{id}/void                                    -> 200  void, number INV-2026-00001 kept
POST  /billing/invoices/{id}/void          (again)                   -> 409  "only an issued invoice can be voided; this one is void"
POST  /billing/invoices/{id}/credit-note   (a void doc)              -> 409  "a void invoice has already been cancelled in full; …"
UPDATE billing_invoices SET due_date = CURRENT_DATE - 3   (psql)
GET   /billing/invoices/{cn}                                         -> 200  issued, due 2026-08-03, overdue TRUE
GET   /billing/invoices?status=issued                                -> 200  INV-2026-00002 overdue TRUE
```

Real rows read back with `psql` afterwards: two documents for that tenant
(`INV-2026-00001` void, `INV-2026-00002` issued and flagged a credit note with
its `credits_invoice_id` set), four line rows whose `qty_milli` mirror exactly
(`2000/-1000` against `-2000/1000`) with `unit_price_cents` of `pg_typeof`
**bigint**, and **one** `billing_sequences` row at `next_value` 3 — two numbers
drawn, and the discarded draft left no hole.

Cuts and flags:

- **HUMAN ACTION (still open) — `/billing` is a new top-level route prefix.**
  The production Caddyfile must add it at the next deploy or every billing
  route returns the SPA. The loop does not touch `deploy/`. (Raised at B1.05;
  flagged again at B1.27.)
- **No `If-Match`/`ETag` on the draft `PATCH`.** Two people editing one draft
  at the same instant still lose an edit at the route layer — the store's row
  lock keeps each *write* coherent and stops an edit landing on a document that
  was issued meanwhile (that race is refused `409`), but it cannot merge two
  editors' intentions. Concurrency control for the whole surface is worth doing
  once, not per module; noted for the B1.27 wave review.
- **No line-totals preview route.** `AccountStore::billing_line_totals` is used
  here only to validate before writing. B1.14 asks for live totals in the
  editor and gets them from the `PATCH` response, which is the stored truth; a
  separate preview endpoint would be a second answer to the same question.
- **No `overdue=1` list filter.** The queue asked for the flag *computed*, and
  it is, on every entry; a filter belongs with the overdue view B1.19 builds,
  where "unpaid" also stops meaning "not `paid`".
- **A create answers `200`, not `201`** — unchanged from B1.05, and revisited
  for the whole surface at once or not at all.
- **No web UI** — B1.13–B1.15 own that; nothing in `web/` was touched, so no
  i18n strings were added.
- **Formatting note.** `rustfmt` follows `mod` declarations, so running it on
  `lib.rs` reformats the whole crate; seven unrelated files it touched were
  reverted, and `lib.rs` keeps its pre-existing module order with only the
  additive `pub mod billing_invoices;` line.

Next item: B1.11 (`billing_quotes` + lines — the same line model, shared where
clean, with the draft/sent/accepted/declined/expired lifecycle and its
allowed-transition tests).

## 2026-08-06 — B1.11 quotes: the offer, its numbers, and its lifecycle

The document that precedes the invoice, store-side and complete: migration,
module, and the lifecycle stated once as data. No routes and no UI in this item
(B1.12 accepts a quote into a draft invoice and wire-verifies; B1.15 draws it),
so nothing under `web/` or `products/` was touched and no i18n strings were
added. Shipped:

- **`platform/alo-store/migrations/0105_billing_quotes.sql`** —
  `billing_quotes` + `billing_quote_lines`. Number, `sent_date` and
  `valid_until` exist exactly when the quote is no longer a draft;
  `decided_date` exists exactly when it is closed; an offer can never expire
  before it was made; the customer link is a composite FK inside the tenant.
  Expand-only, two new tables, nothing dropped or rewritten.
- **`platform/alo-store/src/billing_quotes.rs`** — `QuoteStatus`, `NewQuote`,
  `Quote`, `QuoteSummary`, `QuoteDocument`, and the store surface:
  `create/list(status)/read/update/set_lines/delete`, `send`, and
  `accept | decline | expire` over one private `close_billing_quote`.
- **`platform/alo-store/src/billing_line.rs`** — the line model is now shared
  in fact and not only in prose: `LineTable` (the table + the column naming
  its document) owns the read, the single `INSERT` both document types write
  through, and the whole-set `replace`; `LineRow`, `FiguresRow` and
  `group_figures` moved here too. `billing_invoices.rs` was rewired onto it and
  lost its private copies — its four existing suites (22 tests) still pass
  unchanged, which is what makes the move safe rather than hopeful.
- **`billing_sequence.rs`** — `QUOTE_SEQUENCE_KIND` / `QUOTE_NUMBER_PREFIX`; a
  new series is a row, never a migration, exactly as B1.08 promised.

Decisions, recorded as as-built in `docs/design/billing.md`:

- **Quotes count in a series of their own** (`QUO-YYYY-NNNNN`), not the invoice
  series. Sharing it would leave a visible hole in invoice numbering for every
  offer nobody accepted — the precise appearance gaplessness exists to avoid.
  Quotes are still numbered the same transactional way, so no customer can ever
  receive two offers bearing one number.
- **The lifecycle is one pure table** (`QuoteStatus::allowed_next`): `draft →
  sent`, `sent → accepted | declined | expired`. Every write path asks it, and
  the unit tests walk all **twenty-five** ordered pairs — four legal, twenty-one
  refused, including every self-transition (re-sending would draw a second
  number; accepting twice would hide a caller that lost track of the document).
- **The closing states are terminal.** A declined or lapsed offer does not
  reopen: the answer to a change of mind is a new quote, which keeps the
  document the customer holds and the record of what they were offered the same
  thing. Stated in the module doc so B1.15 does not offer a "reopen" button.
- **`valid_days` is snapshotted on the document** (default 30, range 0–365) and
  `valid_until` is derived at send from the database's own `CURRENT_DATE`,
  exactly as an invoice's due date follows its payment terms. A caller never
  supplies either date.
- **Expiry is a fact and a decision.** `Quote::is_expired(today)` is derived on
  every read (a stored flag would be wrong every midnight); moving the quote to
  `expired` is a separate recorded act with a `decided_date`. There is no
  background sweep, and acceptance refuses on **state**, never on a date — a
  tenant honouring an offer three days late is making a decision they are
  entitled to make, and the store must not overrule it.
- **A quote with no lines cannot be sent** (`Validation`), mirroring the empty
  invoice: an offer that says nothing would spend a number.

How it was verified — `cargo fmt`, `SQLX_OFFLINE=true cargo clippy -p alo-store
--all-targets` clean (zero warnings), `cargo test -p alo-store` green against
the local docker Postgres: **163 unit tests** (of which the new quote module
contributes the transition table, the lapse predicate, the validity range and
the status round-trip) plus every integration suite, including the two new ones:

```
billing_quotes_tenancy    2 passed   round trip + wrong tenant on 8 paths
billing_quote_lifecycle   5 passed   send/answer/lapse/series
billing_invoices_tenancy  3 passed   unchanged, after the shared-line rewire
billing_invoice_lifecycle 5 passed   unchanged
billing_invoice_issue     8 passed   unchanged (incl. the 100-iteration race)
billing_credit_notes      6 passed   unchanged
```

What the wire proved that the unit tests could not:

- Sending stamps `QUO-2026-00001`, today's date, and today + the 14 days the
  document was raised with; the row's own CHECKs accept all of it together.
- A refused send (no lines) leaves **no `billing_sequences` row at all** — the
  next real quote is still number one, so an abandoned draft leaves no hole.
- Two quotes and an invoice interleaved leave exactly two counter rows,
  `invoice → next 2` and `quote → next 3`: `QUO-…-00001`, `QUO-…-00002`,
  `INV-…-00001`. The series do not touch.
- A quote aged past its validity reads as lapsed while its stored status is
  still `sent` — nothing closed it behind the tenant's back — and accepting it
  then succeeds.
- Every path a foreign tenant can reach a quote by (read, list, update, lines,
  delete, send, accept, decline, expire) is a clean `NotFound`, and the
  attempts changed nothing; a quote can never be raised for or moved onto
  another tenant's customer.

Cuts and flags:

- **No routes, no UI, no i18n** — B1.12 (accept → draft invoice, wire-verified)
  and B1.13–B1.15 own those. This item is deliberately store-only, so there was
  no new HTTP surface to verify with curl.
- **No `quote_id` link on `billing_invoices` yet.** The design note promises the
  invoice created on acceptance links back to its quote; that column and the
  copy belong to B1.12, where they are exercised, rather than being added here
  unused.
- **No per-quote concurrency test.** Sending draws through the same
  `draw_next` whose 100-iteration race is already proven in
  `billing_invoice_issue`; a second copy of that test would assert the same
  code twice. The quote path takes the document's lock before the counter's, in
  the same order, for the same reason.
- **No revision chain.** "Quote v2" is a new quote today; linking a replacement
  to the offer it supersedes is a real feature, not a status, and is not in the
  B1 list — flagged here rather than invented.
- **A sent quote cannot be edited at all**, not even its note. Consistent with
  every other document the customer holds; if practice shows tenants need to
  correct a typo on an unanswered offer, the honest answer is decline + re-send,
  which leaves both documents readable.
- **HUMAN ACTION (still open) — `/billing` is a new top-level route prefix.**
  Unchanged from B1.05/B1.10: the production Caddyfile must add it at the next
  deploy. The loop does not touch `deploy/`.

Next item: B1.12 (accept-quote → draft invoice copying the lines, linked back
to the quote, store + HTTP, wire-verified).

## 2026-08-06 — B1.12 an accepted offer becomes the invoice for it

Shipped: **acceptance and the invoice are one act**, plus the whole
`/billing/quotes` HTTP surface the acceptance needed to be reachable at all.

- **Migration `0106_billing_quote_invoice_link.sql`** — `billing_invoices.quote_id`
  (nullable), a composite FK `(tenant_id, quote_id) → billing_quotes` so even a
  bug in a `WHERE` clause cannot link across tenants, a CHECK that a credit note
  never carries one (it credits an invoice, not an offer), and a **unique**
  partial index `(tenant_id, quote_id)`: one invoice per accepted offer, ever.
  The column lives on the invoice — the newer document, which knows its own
  origin — rather than on a quote that is frozen the moment it is sent. `NO
  ACTION` on the FK, deliberately: only a draft quote is ever deleted and a
  draft was never accepted, so nothing linked can vanish; `CASCADE` would have
  been actively wrong (it would delete an invoice), and `NO ACTION` is checked
  after the whole cascade, so dropping a tenant still works.
- **Store** — `accept_billing_quote` now takes the quote's row lock, checks the
  transition, raises the draft invoice (`insert_invoice_from_quote`, in
  `billing_invoices.rs`, which is the one file that writes that table), copies
  every line through the shared `Line::copied`, and *then* writes the closing
  transition — all in one transaction, returning `QuoteAcceptance { quote,
  invoice_id }`. `billing_invoice_for_quote` is the read back.
- **HTTP** — new `billing_quotes.rs` (nine routes: list/create/get/patch/delete
  + send/accept/decline/expire) and a new `billing_document.rs` holding the JSON
  shapes an invoice and a quote share (line, totals, the document body, the
  request line body, the server's `today()`); `billing_invoices.rs` was rewired
  onto it, so the two surfaces cannot drift into two shapes for one line.

Decisions worth keeping:

- **Either the offer closes and its invoice exists, or nothing happened.** Two
  separate calls would leave two unrepairable states: an accepted quote with
  nothing to bill it by (acceptance is terminal — no retry could finish the
  job), or a draft invoice for an offer still shown as open. One transaction
  under the quote's lock also means a decline racing an acceptance either lands
  first (and the acceptance is refused) or waits.
- **What is copied, and what is not.** Customer, currency, the customer's own
  reference, and every line unchanged at the price it was offered at, in the
  offer's order — so the totals agree to the cent, VAT breakdown included. Not
  the **note** (a quote's note states the terms of an *offer*, which is untrue
  of a bill) and not the payment terms, which a quote does not carry at all: the
  days an offer stands and the days a bill is owed in are different facts, so
  the customer's current terms are snapshotted as any new invoice's are.
- **The customer is copied, not re-resolved**, so an offer to a customer
  archived since it was sent can still be honoured — exactly as a credit note
  can still be raised for one. Raising a *new* quote for them stays refused.
- **The invoice is a draft.** What was offered is what will be billed, but when,
  and whether in one go, is the tenant's decision; the legal number comes only
  from the ordinary `/issue`, which is also what keeps the invoice series
  untouched by an offer nobody accepted.
- **`POST /billing/quotes/{id}/accept` answers two documents** (`quote` and
  `invoice`), rendered by the invoice surface's own serializer, so a client
  never has to ask whether one was raised. `GET /billing/quotes/{id}` answers
  `invoiceId` (null unless accepted) — the link B1.15 follows.

How it was verified — `cargo fmt`, `SQLX_OFFLINE=true cargo clippy -p alo-store
-p alo-jmap --all-targets` clean (zero warnings), `cargo test -p alo-store -p
alo-jmap` fully green against the local docker Postgres (109 + 164 unit tests
and every integration suite, exit 0). Two new suites, both passing on the first
run:

- `platform/alo-store/tests/billing_quote_to_invoice.rs` (4) — the done-when
  (an editable draft, hand-computed totals equal to the offer's including the
  per-rate breakdown, every line copied in order with an id of its own, the
  discount line copied as a discount, header copied where the offer decided it
  and current where it said nothing, the link readable from both ends, then the
  draft edited and issued as `INV-2026-00001` while the offer's own lines stay
  as they were); billed **once and only when accepted** (draft/declined/expired
  quotes raise nothing, a second acceptance is refused and raises no second
  document, and deleting the draft invoice leaves the offer accepted); an offer
  to a since-archived customer still honoured while a new offer to them is
  still refused; and the wrong-tenant proof — B cannot accept A's offer, no
  invoice row exists after the refusal, and B cannot read, edit, issue or
  delete the invoice A's acceptance produced.
- `products/mail/alo-jmap/tests/billing_quote_http.rs` (6) — the same arc
  through the real router, plus the frozen-document refusals, the strict status
  filter, all nine routes `401` without a token and `404` with one for an id
  that was never raised, the lapse flag planted with SQL (readable as lapsed
  while still `sent`, and still acceptable), and the mandatory wrong-tenant
  pass over every route (always `404`, never `409` — which would confirm the id
  exists and leak its state — with no refusal echoing the reference it refused,
  and B's number series untouched by A's attempts).

Wire-verified with real curl against the local debug `alo-jmap` on
`127.0.0.1:8080` over docker `alo-pg` (fresh tenant `wireq112`), full
transcript:

```
GET/POST/PATCH/DELETE /billing/quotes[/x][/send|accept|decline|expire]
                                       (no token, 9 routes)   -> 401
POST   /billing/customers                                     -> 200 (EUR, 30-day terms)
POST   /billing/quotes    3 lines, 2 rates, 1 discount        -> 200 draft, number=null
       totals: net 84 247 / VAT 17 333 / gross 101 580
               [{900: net 2 997, vat 270}, {2100: net 81 250, vat 17 063}]
POST   /billing/quotes/{id}/send                              -> 200 QUO-2026-00001
       sentDate=2026-08-06 validUntil=2026-08-20 expired=false
PATCH  /billing/quotes/{id}            (now frozen)           -> 409 "…only…while it is a draft; this one is sent"
DELETE /billing/quotes/{id}            (now frozen)           -> 409 same
POST   /billing/quotes/{id}/send       (again)                -> 409 "…cannot become sent while it is sent; from sent it can only become accepted or declined or expired"
POST   /billing/quotes/{id}/accept                            -> 200 TWO documents
       quote  : status=accepted decidedDate=2026-08-06 number=QUO-2026-00001
       invoice: status=draft number=null quoteId=<the quote>
                currency=EUR paymentTermsDays=30 reference=RFQ-2026-88 note=""
       totals : IDENTICAL to the quote's, per rate and in total
       lines  : IDENTICAL values, same order (incl. the -1 000 discount)
POST   /billing/quotes/{id}/accept     (again)                -> 409 "…it is closed and cannot change again"
GET    /billing/quotes/{id}                                   -> 200 invoiceId=<the invoice>
GET    /billing/invoices/{id}                                 -> 200 quoteId=<the quote>
POST   /billing/invoices/{id}/issue                           -> 200 INV-2026-00001
       issueDate=2026-08-06 dueDate=2026-09-05 quoteId kept, gross 101 580
POST   /billing/quotes                 (no customerId)        -> 422 "customerId is required to raise a quote"
POST   /billing/quotes                 (unknown customer)     -> 404
POST   /billing/quotes                 (19.99 in a cents field) -> 400 "malformed request body"
POST   /billing/quotes/{empty}/send                           -> 422 "a quote with no lines cannot be sent…"
POST   /billing/quotes/{draft}/accept                         -> 409 "…from draft it can only become sent"
PATCH  /billing/quotes/{draft}         validDays=400          -> 422 "…between 0 and 365 days"
GET    /billing/quotes?status=issued                          -> 422 "status must be one of draft, sent, accepted, declined, expired"
GET    /billing/quotes/nope                                   -> 404 "no such quote"
DELETE /billing/quotes/{draft}                                -> 200
POST   /billing/quotes/{id}/decline                           -> 200 declined, decidedDate stamped, NO invoice in the body
POST   /billing/quotes/{id}/expire                            -> 200 expired, decidedDate stamped
GET    /billing/quotes/{declined|expired}                     -> 200 invoiceId=null (neither was billed)
GET    /billing/quotes                                        -> [QUO-…-00003 expired, …00002 declined, …00001 accepted]
GET    /billing/invoices                                      -> [INV-2026-00001 issued, from QUO-2026-00001, 101 580]
```

The database was read directly afterwards: `billing_invoices.quote_id` is
present with the unique partial index `billing_invoices_from_quote`, the
composite FK to `billing_quotes`, and the credit-note CHECK — and every stored
invoice with an origin joins to exactly one accepted quote.

Cuts and flags:

- **No UI, no i18n** — B1.13–B1.15 own the screens; this item deliberately ends
  at the wire. No user-facing strings were added.
- **No `POST /billing/quotes/{id}/duplicate` or revision chain.** "Quote v2" is
  still a new quote (flagged in B1.11, unchanged).
- **Partial invoicing of one offer is not possible**: the unique index means one
  invoice per accepted quote. Deliberate — a caller that wants to bill an offer
  in stages edits the draft or raises further invoices by hand, and "bill 40 %
  now" is a milestone feature, not a property of acceptance. Flagged rather than
  invented.
- **The invoice's `quoteId` is not writable by any request** and is not part of
  `NewInvoice`; it is stamped by acceptance and kept through issue.
- **HUMAN ACTION (still open) — `/billing` is a new top-level route prefix.**
  Unchanged from B1.05/B1.10: the production Caddyfile must add it at the next
  deploy. The loop does not touch `deploy/`. The new routes are all under that
  one prefix, so nothing further is needed.

Next item: B1.13 (web: the Billing module skeleton — rail entry, `/billing`
routes, customer and product list pages with create/edit dialogs, i18n en).

---

## 2026-08-06 — B1.13 the Billing module, on screen for the first time

Fourteen items of server work became something a person can use. Shipped
`web/src/billing` — a rail module of the **workspace product only**
(`product/workplace.tsx`; alomails is deliberately untouched, and a build with
`ALO_PRODUCT=mail` was checked to contain no billing code at all) mounted at
`/billing/*` with a tab per record type:

- **`BillingModule.tsx`** — header, the `customers` / `products` tabs, nested
  routes, and `/billing` landing on customers. Later items add a tab, never a
  second navigation idea.
- **`CustomersView.tsx` / `ProductsView.tsx`** — the two lists: search,
  "show archived", a create action, a row that opens its record, and an
  archive/restore action per row. Neither list has a delete, because the store
  has none: archiving is the only removal.
- **`CustomerDialog.tsx` / `ProductDialog.tsx`** — the create/edit forms.
- **`api.ts`** — the `/billing` client, **its own small class rather than more
  methods on `JmapClient`**: billing is plain REST with none of JMAP's session,
  capabilities or method-call envelope, `client.ts` is already 2 300 lines of
  mail, and the two change for entirely different reasons. It takes the auth
  layer's `authorizedFetch`, so there is still exactly one session.
- **`money.ts`** — the one place typing becomes money (below).
- **`types.ts`**, **`parts.tsx`** (the toolbar/empty/error/dialog chrome both
  pages share, so customers and the price list cannot drift into two
  different-looking screens), **`BillingModule.module.css`**, **`index.ts`**.

Plus: `moduleBilling` and 60 `billing*` keys in `i18n/en.ts` (fr/nl at B1.27,
as the queue schedules), the `Receipt` rail entry, and `/billing` added to the
Vite dev proxy's `API_PATHS` — without that line the dev server answers billing
XHRs with its own index.html and nothing works in `npm run dev`.

Three decisions worth keeping:

- **No validation lives in the client**, exactly as B1.05 decided for the route
  layer. A form sends what was typed; a `422` is shown **in the server's own
  words**, next to a form that stays open holding everything the user entered,
  so it can be fixed in place. The store authors those messages to name the
  broken rule and never to echo stored data, which is what makes them safe to
  display. The single client-side refusal is text that is not a number at all.
- **No money is computed in the browser** (law 2 of the design note).
  `money.ts` does one conversion in each direction: a typed decimal into whole
  hundredths — cents for a price, basis points for a rate, one function because
  they are the same arithmetic — and back. The parse rule is
  locale-independent, because a Dutch user with an English UI still types Dutch
  numbers: with both separators the last one is the decimal (`1.234,56` and
  `1,234.56` are both 123456); with one, followed by one or two digits, it is
  the decimal; otherwise it is grouping and the integer part must really be
  grouped in threes (`1.500` is 150000, `1.2345` is refused rather than read as
  a number nobody typed). A third decimal is refused, never rounded away. The
  cents are assembled from the two integer halves rather than parsed as a
  float, so a price never depends on whether `1.15 * 100` lands on 115.
- **An edit sends only the fields that changed.** The surface is
  last-writer-wins (B1.05: no `ETag` yet), so writing back every field would
  make a one-field edit clobber a colleague's concurrent one. A cleared text
  box sends an explicit `null` — the door B1.05's `absent_or_null` opened, and
  the only way a VAT id ever comes off a customer.

Verified. `npx tsc --noEmit` clean, `npx eslint src --max-warnings 0` clean
across the whole app (not only the new files), `npm run test` green — 104 tests
in 17 files, 22 of them new — and `npm run build` clean for both the workspace
and the `ALO_PRODUCT=mail` product.

The new tests are the item's real gate, since a type checker cannot see wiring:

- `money.test.ts` (15) pins every parse rule including the refusals: both
  notations, the mixed-separator forms, grouping vs decimal, spreadsheet
  spaces, the sign (kept, because the *store* owns the rule that refuses a
  negative price — the client must not hold a second definition of valid),
  the float trap (`1.15` → 115, `8.29` → 829), a value too large to stay an
  exact integer, and a round trip through the editable form for every shape.
- `BillingModule.test.tsx` (7) renders the real views with the real client over
  one recording `fetch`: a list shows what the API answered and asks for
  `includeArchived=1` when the toggle goes on; a create sends `{name, country}`
  and nothing else, so the server's own defaults still apply; clearing the VAT
  id sends `{"vatId": null}` on a `PATCH` to the right id; a `422` puts the
  server's sentence in an `alert` with the form still holding what was typed;
  a price typed `1 234,56` and a rate typed `5,5` leave as `unitPriceCents:
  123456` and `vatRateBp: 550`; and a price of "twelve fifty" is never sent at
  all.

Wire-verified against the **local** backend through the **Vite dev server** —
the browser's actual path, not curl straight at the API: docker `alo-pg`, the
debug `alo-jmap` on `127.0.0.1:8080`, a fresh tenant `wireb113`, a real PKCE
token, and every call made to `localhost:5173`:

```
GET  /billing, /billing/customers, /billing/products  (Accept: text/html)
                                                   -> 200 Vite SPA shell (deep links reload)
GET  /billing/customers          (no token)        -> 401 missing or invalid bearer token
POST /billing/products           (no token)        -> 401 same
GET  /billing/customers                            -> 200 0 rows
POST /billing/customers   "  Nordwind Handel GmbH  ", de, " de 811.907-980 ", 14d, eur
                                                   -> 200 'Nordwind Handel GmbH' DE DE811907980 EUR 14d
POST /billing/customers   vatId DE811907981        -> 422 "the check digit of this DE VAT id
                                                          does not match; check for a typo"
PATCH/billing/customers/{id} {city, paymentTermsDays}  -> 200 city+terms changed, vatId kept
PATCH/billing/customers/{id} {"vatId":null}        -> 200 vatId null
POST /billing/customers/{id}/archive true          -> 200 archived
GET  /billing/customers                            -> 200 0 rows
GET  /billing/customers?includeArchived=1          -> 200 2 rows
POST /billing/customers/{id}/archive false         -> 200 restored
GET  /billing/customers/no-such-id                 -> 404 "no such customer"
POST /billing/products    (price "1 234,56", rate "5,5" as the dialog parses them)
                                                   -> 200 unitPriceCents=123456 vatRateBp=550
POST /billing/products    19.99 in a cents field   -> 400 "malformed request body"
PATCH/billing/products/{id} {"unitPriceCents":13000}   -> 200 price changed, name+rate intact
POST /billing/products/{id}/archive true/false     -> 200 archived, then restored
GET  /billing/products / ?includeArchived=1        -> 200 0 rows / 1 row
```

One observation from that transcript, recorded rather than acted on: a create
with a blank name **and** no country answers about the country first, because
that is the order `normalize()` checks fields in. Correct, just worth knowing
when reading a refusal.

Cuts and flags:

- **No browser click-path was exercised.** There is no headless browser in this
  environment, so "it renders and the buttons work" is proven by the component
  tests above (real views, real client, real parsing, fake network) plus the
  wire transcript through the dev server — not by clicking. B1.15's done-when
  asks for a manual click-path; that is a human step and is flagged here.
- **No contact link on a customer.** `contactId` exists in the API and the
  type, but picking an address-book contact needs a contact picker that is not
  this item's; the field is simply not on the form yet.
- **No tenant currency setting, so the price list shows a bare amount** (no
  symbol). A price list is quoted in the tenant's own currency, and nothing
  stores that yet; inventing "EUR" in the UI would be stating a fact we do not
  have. The customer list shows its own currency code per row.
- **Country and currency are free-text boxes**, not pickers — the store
  validates them by shape rather than against an ISO list (B1.02's recorded
  cut), and a picker would need exactly the list that cut refused to invent.
- **Server refusals are English.** The `422` detail comes from the store, which
  is not translated; the fr/nl pass at B1.27 covers the module's own strings
  but cannot cover those. **Flagged for human review**: either the store starts
  returning a machine-readable rule code the client translates (a contract
  change, additive), or refusals stay English. Not a decision to slip in
  quietly under a UI item.
- **HUMAN ACTION (still open) — `/billing` is a new top-level route prefix.**
  Unchanged from B1.05/B1.10/B1.12: the production Caddyfile must add it at the
  next deploy, or every billing XHR gets the SPA. `vite.config.ts` (dev only,
  not `deploy/`) now has it.

Next item: B1.14 (web: the invoice list with status chips and overdue
highlighting, plus the draft editor with live totals from the server).

---

## 2026-08-07 — B1.14 the invoice list and the draft editor

The documents themselves are now on screen. `web/src/billing` gained a
`invoices` tab (first, ahead of customers and the price list — documents are
what the module is *for*, and `/billing` now lands there), a list, and an
editor:

- **`InvoicesView.tsx`** — number, customer, issue and due dates, state chips
  and the total. Two things it deliberately does not do: it never adds
  anything up (the gross in the last column is `totals.grossCents` off the
  list entry) and it never decides what is late (`overdue` is the server's,
  computed against the server's date — a browser with a wrong clock cannot
  clear or invent an overdue invoice). The status filter is the **server's**
  `?status=`, not a filter over the loaded page: a bookkeeper asking for
  issued documents must get the tenant's issued documents, not the issued ones
  out of the first screenful.
- **`InvoiceEditor.tsx`** — one document. `new` raises a draft for the picked
  customer (that is all the store needs; currency and payment term are read
  from the customer and snapshotted), then the same screen becomes the editor
  for the saved draft.
- **`InvoiceLines.tsx`** + **`lineRows.ts`** — the line grid and, separately,
  the pure model behind it: which typed row becomes which integers, and when
  the whole set must not be sent at all. Kept apart from the rendering because
  it is where a wrong invoice would be written, and it is unit-tested on its
  own.
- **`TotalsPanel.tsx`**, **`status.tsx`**, **`dates.ts`** — the money block,
  the state chips, and the one place a calendar day becomes readable.

Four decisions worth recording.

- **The totals on screen are always a server response.** The draft **saves
  itself** 700 ms after typing stops, and the document that comes back is what
  the totals panel and the per-line nets render. Between a keystroke and that
  response the figures are the previous ones — dimmed, with a line saying so —
  because the alternative is a browser computing money, which this module does
  not do. That is also why a draft is raised *before* it is lined: without a
  document id there is nothing to ask the server what the lines come to.
- **The save is a loop, not a queue.** One request may be in flight at a time;
  an edit that lands mid-save bumps a counter, and the save loop goes round
  again with the newest form instead of racing it. A save that finishes into a
  changed form never reports "saved".
- **A row that is not yet a line holds the save.** The API replaces the whole
  line set in one write, so a set that quietly left the offending row out would
  *delete* the line it stands for. A wholly untouched row (added, then
  abandoned) is dropped; anything with a keystroke in it must become a line,
  and the row says which field is stopping it. This is the only client-side
  refusal, alongside "that is not a number" — every other rule stays the
  store's.
- **Quantities take no grouping separator.** `money.ts` now parses any fixed
  scale (`parseScaled`), with hundredths for money and **milli-units** for a
  quantity. `1.500` as an amount is unambiguously fifteen hundred, but as a
  quantity it is one-and-a-half as often as it is fifteen hundred, and `0.125`
  hours has to stay writable — so a quantity reads every separator as the
  decimal point and refuses a grouped integer part. A document is never billed
  a thousand times what someone typed.

**The wire found a real defect, and it was fixed before the item closed.** The
first cut of the editor sent the whole header on every autosave. Against the
local backend that turned out to make a draft *uneditable* for ever if its
customer was archived after the document was raised: restating `customerId`
sends the document back through the store's customer check, which answers
`422 the customer is archived`. The editor now sends **only the header fields
that changed** (the module's own stated rule, B1.13), always with the line set.
Both paths are now proven on the wire, and pinned by a test.

Verified. `npx tsc --noEmit` clean on both tsconfigs, `npx eslint . --max-warnings 0`
clean across the whole app, `npm run test` green (127 tests in 19 files, 23 of
them new), `npm run build` clean for the workspace **and** for
`ALO_PRODUCT=mail`.

The new tests are the item's real gate:

- `lineRows.test.ts` (8) — a stored line round-trips without a figure moving; a
  picked price-list item is a snapshot (price and rate copied, the typed
  quantity kept); a line nobody gave a number to bills **once**, and one nobody
  priced is free; each row reports its first problem in field order; the set
  drops untouched rows, keeps the order of the rest, and refuses to be sent
  while one row is not a line.
- `Invoices.test.tsx` (10) — the real router, the real module routes, the real
  client, one recording `fetch`: the list shows the server's number, customer
  and gross and marks what is late; the status filter goes to the server; a
  draft is raised with `{customerId}` and nothing else; a typed `2` leaves as
  `qtyMilli: 2000` and a typed `1 234,56` as `unitPriceCents: 123456`; the
  totals rendered after a save are the server's **even when they do not match
  the lines** (the response says `99999`, the screen says €999.99); a changed
  reference is sent alone, without `customerId`; a row without a description
  produces no request at all and says why; picking a price-list item sends the
  copied price, rate and a quantity of one; a `422` lands in an `alert` in the
  server's own words with the form intact; and an issued document offers no
  inputs, no add-line and no delete, with its figures formatted from the
  document rather than from a form.
- `money.test.ts` (+5) — the quantity scale: three decimals in either
  notation, a separator never read as grouping, the refusals, the round trip,
  and the reading format.

Wire-verified against the **local** backend (docker `alo-pg`, the debug
`alo-jmap` on `127.0.0.1:8080`, fresh tenant `wireb114`, real password-grant
token). Every request below is byte-for-byte what these screens send:

```
GET   /billing/invoices, /billing/invoices/{id}   (no token)  -> 401
POST  /billing/customers, /billing/products                   -> 200 (the editor's two pickers)
POST  /billing/invoices  {"customerId"}    ← "Create draft"    -> 200 status=draft number=null
                                              lines=[] totals={0,0,0,[]} currency=EUR terms=14
PATCH /billing/invoices/{id} {"lines":[2 lines, 2 rates]} ← autosave
                                              -> 200 lines: Consulting 1500×12500 net 18750
                                                            Travel     1000× 4990 net  4990
                                                 totals: net 23 740 / VAT 4 287 / gross 28 027
                                                 vatByRate [{700: net 4 990, vat 349},
                                                            {2100: net 18 750, vat 3 938}]
GET   /billing/invoices                                       -> 200 summary carries totals,
                                                 overdue, creditNote, currency — and NO lines
GET   /billing/invoices?status=draft                          -> 200
GET   /billing/invoices?status=issued                         -> 200 0 rows
GET   /billing/invoices?status=bogus                          -> 422
GET   /billing/invoices/{id}                                  -> 200 {invoice(+lines), creditNotes:[]}
POST  /billing/invoices/{id}/issue                            -> 200 INV-2026-00001
                                                 issueDate=2026-08-06 dueDate=2026-08-20
PATCH /billing/invoices/{id}  (now issued)                    -> 409 "an invoice can only be
                                                 changed while it is a draft; this one is issued"
DELETE/billing/invoices/{draft}                               -> 200, then GET -> 404
POST  /billing/invoices {lines:[{description:""}]}            -> 422 "line 1: description must not be empty"

  the archived-customer path, before and after the fix:
PATCH /billing/invoices/{id} {customerId, reference, note, lines}   (customer archived
                                                 after the draft was raised) -> 422
PATCH /billing/invoices/{id} {"lines":[…]}        (same document)  -> 200   ← what ships
POST  /billing/invoices {"customerId": <archived>}                 -> 422 (still refused,
                                                 which is correct: that is a new document)
```

Cuts and flags:

- **No browser click-path was exercised.** No headless browser in this
  environment (unchanged from B1.13). "It renders and the buttons work" is
  proven by the component tests above — real router, real views, real client,
  real line model, fake network — plus the wire transcript. B1.15's done-when
  asks for a manual click-path; that remains a human step.
- **Issuing, voiding, crediting and printing are not in this item** (B1.15,
  B1.16). A document that carries a number therefore renders as a frozen
  record: the editor shows it read-only with the reason, rather than offering
  edits the store would refuse. The routes exist and are wire-verified above;
  only the buttons are missing.
- **The currency and the payment term are not editable on the document.** Both
  are snapshotted from the customer when the draft is raised; the API can take
  them, but "what this document is denominated in" is not a text box, and
  changing it after lines exist wants a rule the design note has not written.
  Both are shown, the term as the due-date hint.
- **The line set is rewritten whole on every save**, so stored line ids change
  each time. Harmless today (row identity is the editor's own, and the per-line
  net is paired by print order), but worth knowing when B1.19 attaches anything
  to a line.
- **A save never merges**: last writer wins, as everywhere in this module
  (no `ETag` yet, B1.05). Two people in the same draft will overwrite each
  other's lines, which is a document-locking decision, not a UI one.
- **Server refusals are English** (unchanged from B1.13, flagged for human
  review there and still open).
- **HUMAN ACTION (still open) — `/billing` is a new top-level route prefix.**
  Unchanged from B1.05/B1.10/B1.12/B1.13: the production Caddyfile must add it
  at the next deploy, or every billing XHR gets the SPA. No new prefix in this
  item.

Next item: B1.15 (web: the issue flow with its confirm dialog, the read-only
issued view, the credit-note button, and the quote pages mirroring invoices
including accept → invoice).

---

## 2026-08-07 — B1.15 the issue flow, credit notes, and the quote screens

The lifecycle a document has is now something a person can drive. Two new
screens, three new buttons on the invoice, and — the part worth reading — the
two editors became **one** editor.

- **`documentDraft.ts`** (new) — the form behind any billing document: load,
  adopt, the edit state, and the autosave loop (one request in flight, an edit
  mid-save goes round again, only the header fields that actually changed are
  sent). Lifted verbatim out of `InvoiceEditor`.
- **`DocumentEditor.tsx`** (new) — the shell both editors render: header
  fields, line grid, totals, save indicator, create bar, and the read-only
  rendering of a numbered document. What varies is passed in — the words, the
  two dates, the chips, the transitions, the relations.
- **`DocumentActions.tsx`** (new) — the transitions, each behind a dialog that
  says what it does to the document.
- **`InvoiceLines.tsx` → `DocumentLines.tsx`** — the grid was already generic;
  only its name said "invoice", and a quote's lines are the same object.
- **`pickers.ts`** (new) — the customers and the price list, loaded once per
  screen instead of once per editor.
- **`QuotesView.tsx` / `QuoteEditor.tsx`** (new), the `quotes` tab and its
  routes, `QuoteChips` in `status.tsx`, the quote types, and eleven new client
  methods (`issueInvoice`, `voidInvoice`, `createCreditNote`, the seven quote
  ones) — plus `#act`, a `POST` that carries no body at all.
- 40 new `billing*` keys in `i18n/en.ts` (fr/nl at B1.27, as scheduled).

`InvoiceEditor` went from 457 lines to 211, all of them about what an invoice
is; `QuoteEditor` is 190 and is not a copy of it. Had the quote screen been
written as a sibling, the wave would carry two autosave loops and two
definitions of "only send what changed" — and B1.19's payments would have to
fix both.

Decisions worth recording, all now as-built in `docs/design/billing.md`:

- **A transition waits for the form.** Every lifecycle button is disabled, with
  a line saying why, while the draft holds edits the server has not stored.
  This was found by reading the diff cold, not by a test failing: a transition
  acts on the **stored** document, so firing one mid-autosave would freeze a
  document that is not the one on screen and lose the keystrokes since the last
  save *inside a document nobody can edit again*. A row that cannot become a
  line keeps the buttons disabled indefinitely, which is correct — a document
  whose editor holds an unsendable line is not one to issue.
- **The dialog states what the action does, never "are you sure".** Issuing
  "takes the next number in your series, dates the document and freezes it…";
  sending a quote "takes the next quote number… so what the customer holds
  cannot change under them". Each of these is irreversible on a legal document,
  and the item's own done-when asked for exactly this sentence.
- **A transition request carries no body**, asserted on the wire and in the
  tests. What a document becomes is the route, never a field a stale form could
  send.
- **Where a transition answers with a different document, the screen goes
  there.** Accepting a quote lands on the draft invoice; raising a credit note
  lands on the credit note. Both are the document that now needs work. Both
  directions of every link are on the record (an invoice names the quote it
  came from and the invoice it credits; a quote names the invoice it became).
- **"Past its date" ≠ "Expired".** The computed `expired` flag and the
  `expired` status are different facts, so they are different words. A lapsed
  offer still offers Accept — the store refuses on state, never on a date, and
  this screen must not lock a door the store leaves open.
- **Invoices stay the first tab and stay what `/billing` lands on**, with
  quotes second. B1.13/B1.14 recorded why; a new tab is not a reason to
  relitigate it.
- **"Sent on", not "Sent", as the quote list's date column** — the same reason
  the invoice list says "Issue date": a header that reads like the chip under
  it makes a list ambiguous at a glance. Caught by a test finding two "Sent"s.

Verified. `npx tsc --noEmit` clean on both tsconfigs, `npx eslint . --max-warnings 0`
clean across the whole app, `npm run test` green — **144 tests in 20 files, 28
of them new** — and `npm run build` clean for the workspace **and** for
`ALO_PRODUCT=mail`, whose bundle was probed and contains no billing route, no
billing component and no billing CSS (only the shared string catalogue, as
before this item).

The new tests are the item's gate, all through the real router, the real module
routes, the real client and the real shared shell over one recording `fetch`:

- `Invoices.test.tsx` (+6) — issuing shows the dialog's **own words**, writes
  nothing when the dialog is cancelled, sends a bodiless `POST …/issue`, and
  then renders the server's frozen document (number, notice, no inputs, no
  Issue button); each status offers only its own transitions and a void
  document offers none; a credit note lands on the editable mirror, marked as a
  credit note, worth `-€226.88` — the server's figure — and naming the invoice
  it credits; voiding keeps the number; a refused transition is shown in the
  server's words with the draft still editable; and the new guard: with a row
  that cannot become a line, the Issue button is disabled, clicking it opens no
  dialog and, 1.5 s later (two debounces), nothing has been written.
- `Quotes.test.tsx` (new, 11) — the list shows the server's number, customer,
  gross and dates and flags what has lapsed without calling it Expired; the
  status filter goes to the server; a draft is raised with `{customerId}` and
  nothing else; a typed quantity leaves as `qtyMilli` and the totals rendered
  are the server's even when they do not match the lines; only the changed
  header field is sent; each state offers only its own transitions and a closed
  offer none; sending is cancellable and then bodiless and freezing; **accepting
  posts to `…/accept` and really lands on the draft invoice `inv-9`** (the GET
  for it is in the recorded calls), which names the quote it came from — and
  that link goes back to the accepted offer; declining closes it keeping its
  number; an accepted offer names the invoice it became; a refusal is shown in
  the server's words.

Wire-verified against the **local** backend (docker `alo-pg`, the debug
`alo-jmap` on `127.0.0.1:8080`, fresh tenant `wireb115`). Every one of the
seven new routes `401`s without a token. The arc, abbreviated:

```
POST   /billing/invoices {customerId}                 -> 200 draft, number null
PATCH  /billing/invoices/{id} {lines:[…]}             -> 200 net 18 750
POST   /billing/invoices/{id}/issue                   -> 200 INV-2026-00001
                                          issueDate 2026-08-06, dueDate 2026-08-20
PATCH  /billing/invoices/{id}                         -> 409 "…only…while it is a draft; this one is issued"
DELETE /billing/invoices/{id}                         -> 409 same
POST   /billing/invoices/{id}/issue   (again)         -> 409 "…can only be issued while it is a draft"
POST   /billing/invoices/{id}/credit-note             -> 200 draft, creditNote=true,
                                          creditsInvoiceId={id}, every qty negated
GET    /billing/invoices/{original}                   -> 200 creditNotes:[the draft]
POST   /billing/invoices/{credit}/issue               -> 200 INV-2026-00002  (one series)
POST   /billing/invoices/{other}/issue                -> 200 INV-2026-00003
POST   /billing/invoices/{other}/void                 -> 200 void, number kept
POST   /billing/invoices/{other}/void  (again)        -> 409 "only an issued invoice can be voided"

POST   /billing/quotes {customerId}                   -> 200 draft
PATCH  /billing/quotes/{id} {lines:[2, incl. a -1 discount]} -> 200
POST   /billing/quotes/{id}/send                      -> 200 QUO-2026-00001
                                          sentDate 2026-08-06, validUntil 2026-09-05
PATCH  /billing/quotes/{id} / DELETE                  -> 409 "…this one is sent"
POST   /billing/quotes/{id}/send      (again)         -> 409 "…from sent it can only become
                                          accepted or declined or expired"
POST   /billing/quotes/{id}/accept                    -> 200 TWO documents; invoice is a
                                          draft with the same lines and totals
POST   /billing/quotes/{id}/accept    (again)         -> 409 "…it is closed and cannot change again"
GET    /billing/quotes/{id}                           -> 200 invoiceId={the invoice}
GET    /billing/invoices/{that}                       -> 200 quoteId={the quote}
POST   /billing/quotes/{empty}/send                   -> 422 "a quote with no lines cannot be sent"
POST   /billing/quotes/{sent}/decline | /expire       -> 200 decidedDate stamped
POST   /billing/quotes/{declined}/accept              -> 409 "…while it is declined; it is closed"
GET    /billing/quotes?status=accepted                -> 200 1;  ?status=bogus -> 422
```

Read back from the database afterwards: three invoices on **one** gapless
series (`INV-2026-00001` issued, `-00002` the credit note, `-00003` void), the
accepted quote's invoice still a numberless draft whose lines sum to 17 750 —
exactly the offer, discount included — three quotes on their own series, and
both counters at 4. The two series are separate rows (`invoice`, `quote`) of
`billing_sequences`, as B1.08/B1.11 designed.

Then through the **Vite dev server** (the browser's actual path, not curl at
the API): `/billing`, `/billing/quotes`, `/billing/quotes/new`,
`/billing/quotes/{id}` and `/billing/invoices/{id}` all serve the SPA shell on
an HTML navigation — deep links survive a reload — while the same paths as XHR
proxy to the API (`?status=accepted` 200, `?status=bogus` 422, a bodiless
`POST …/accept` without a token 401). No `vite.config.ts` change was needed:
`/billing` has been in `API_PATHS` since B1.13 and the new routes are under it.

Cuts and flags:

- **No browser click-path was exercised** (unchanged from B1.13/B1.14: there is
  no headless browser in this environment). The item's done-when asks for a
  manual click-path; that remains a **human step**. What stands in for it is
  above: 28 component tests driving the real screens through their real
  routers, the full curl arc, the database read back, and the dev-server pass.
- **A quote's validity is not editable on the document** — shown, not typed,
  exactly as the payment term is on an invoice (B1.14's recorded cut). The API
  takes `validDays`; making it a text box wants the same rule "what this
  document is denominated in is not a text box" already settled for currency.
  Additive to add later.
- **No `paid` transition anywhere**, because there is no payment yet: B1.19
  owns `billing_payments` and the derived paid state. The `paid` status is
  handled everywhere it can appear (a paid invoice offers a credit note and not
  a void), which is what the store already allows.
- **The credit-note screen offers no "credit the rest" arithmetic.** A partial
  credit is made by editing the mirror down, as the store models it; nothing
  totals up what has already been credited, because the over-credit guard is
  the same derived-state machinery B1.19 builds (flagged at B1.09, unchanged).
- **Server refusals are English** (flagged for human review at B1.13, still
  open, and now visible in one more place: a refused transition).
- **Honest note:** one of ~13 full `npm run test` runs printed
  `Errors  1 error` alongside 144 passing tests, with no failing test and no
  message identifying it; twelve further runs were clean and it did not
  reproduce. Recorded rather than swept up — if it returns, it is a real
  unhandled rejection to chase, most likely a request settling after unmount.
- **HUMAN ACTION (still open) — `/billing` is a new top-level route prefix.**
  Unchanged from B1.05/B1.10/B1.12/B1.13/B1.14: the production Caddyfile must
  add it at the next deploy, or every billing XHR gets the SPA. This item adds
  no new prefix.

Next item: B1.16 (the invoice/quote HTML print view — a branded document with
addresses, lines, VAT breakdown, payment terms and bank details, print-optimised
to a correct A4 page, which is also the PDF source).

*Correction, same iteration:* commit `d8b29c6` went out **without** the
`Co-Authored-By: Claude …` trailer — the same slip as `0364163` at B1.09, and
the same cause: a commit message written with `git commit -F -` does not get
the trailer the harness would otherwise append, so it has to be written into
the message by hand. It was already pushed when this was noticed, and rewriting
pushed history is forbidden by the loop's safety rails, so the commit stands and
the gap is recorded here. The authorship itself is correct (the repository
owner, as configured). Every later commit writes the trailer explicitly.

---

## 2026-08-07 — B1.16 the printed document, and who it is from

The paper a customer actually receives. `GET /billing/invoices/{id}/print` and
`GET /billing/quotes/{id}/print` answer **one self-contained HTML page** — no
script, no font, no image, no request of any kind — laid out for A4 by its own
`@page` rules.

**The decision that shaped the item, recorded in `docs/design/billing.md`
before any code: the page is rendered on the server.** The alternative was the
React module composing it. Three things ruled that out — it is the source of
the PDF (B1.17) and of the mail attachment (B1.18), neither of which has a
browser session to render in, so a client-rendered document would have to be
written twice and the two would drift; and a document assembled from the app's
`ds` tokens inherits the app's layout instead of an A4 sheet. The browser
therefore *fetches* the document with the session's bearer token and prints it,
rather than composing it.

Shipped:

- **`platform/alo-store/src/iban.rs`** (new) — ISO 13616 length per country
  (79 registered countries) **and** the ISO 7064 mod-97 check, carried digit by
  digit so no integer type is involved; plus BIC shape. `grouped()` is the
  spaced form a document prints. A typo'd IBAN is money that never arrives and
  is caught at the point of entry or not at all.
- **Migration 0107 + `billing_settings.rs`** (new, store) — the issuer side of
  every document: **one row per tenant**, legal name required, address, country,
  VAT id, register number, contact, IBAN/BIC/bank/holder, footer note. A tenant
  that has never saved reads the **blanks**, never a `404`; `is_stated()` tells
  the two apart. `billing_field::country` was lifted out of `billing_customers`
  so both records hold one country rule (the customer's is required, the
  issuer's may be unstated).
- **`billing_settings.rs`** (new, jmap) — `GET` + `PATCH /billing/settings`. A
  `PATCH`, not a `PUT`, because it behaves like one: absent keeps, `null`
  clears, and a `PUT` would promise a whole-document replace and quietly blank
  what an older client did not know to send.
- **`billing_print.rs`** (new, jmap) — the renderer, its `Strings` table, and
  the response. `PrintDocument` is what an invoice, a credit note and a quote
  all reduce to, so the three are one layout with different words rather than
  three that look alike.
- **Web** — a `Print` button on both editors, `printSheet.ts`, the `Your
  details` tab (`SettingsView.tsx`), `documentHtml`/`settings`/`saveSettings`
  on the client, and 31 new `billing*` keys in `i18n/en.ts` (fr/nl at B1.27).

Decisions worth recording, all now as-built in `docs/design/billing.md`:

- **The document says what it is.** A draft prints as a draft **and carries no
  number** (it has none); a void invoice prints as void; a credit note is
  titled as one and names the invoice it corrects. Paper that could be mistaken
  for an issued invoice is a legal problem, not a cosmetic one.
- **A quote and a credit note print no bank details**, and say explicitly that
  nothing is payable. An IBAN under that sentence is how a document gets paid
  twice. An invoice with no due date yet states the *term* instead, so the page
  never simply omits when the money is owed.
- **Dates are ISO `YYYY-MM-DD` in every language.** `05/03/2026` is two
  different days depending on the reader; a misread due date is a dispute.
- **`srcdoc`, not a blob URL and not a tab.** A tab needs a URL and the route
  is authenticated — an anonymous tab gets a `401`. A `blob:` URL is blocked by
  our own CSP (`frame-src 'self'`), while `srcdoc` inherits the parent policy
  and its `style-src 'unsafe-inline'`, which the document's inline stylesheet
  needs. Asserted in a test, because the reason is invisible in the code.
- **Two different mechanisms keep the page inert, one per way it is used** —
  and the first draft of this item claimed only one, which the cold review
  caught. Fetched **as a document** (headless chromium at B1.17, a saved file,
  a mail client) the response's own `Content-Security-Policy: default-src
  'none'` binds it, with `nosniff` and `no-store`. Mounted by the **web app**
  that header never applies: `srcdoc` is same-origin and inherits the *app's*
  policy. So the frame is **sandboxed without `allow-scripts`**
  (`allow-same-origin` so this window can call `print()` on it, `allow-modals`
  because a print dialog is one). Both mechanisms are now asserted — the
  headers in `billing_print_http.rs`, the sandbox in `Printing.test.tsx` — and
  the code says in both places that neither substitutes for the other.
- **The issuer is read live, not snapshotted at issue.** A reprint shows the
  current address and bank — which is what moving office is supposed to do —
  while number, dates, lines and money live on the document.
- **`?lang=` falls back to the default rather than refusing**, deliberately
  unlike the strict `status` filter: a document that will not print because of
  a display preference is worse than one printed in English.
- **Two things came from looking at the rendered page, not from a test.** The
  number was printed twice (heading *and* meta grid — a reader then checks
  whether the two agree), and a lone `NL` under a Dutch address read like a
  stray field. Both fixed: the number is stated once, and a **domestic** address
  omits its country while a cross-border one keeps it, which is the line that
  decides VAT treatment anyway.
- **`Print` waits for the save**, like every lifecycle button (B1.15): it
  prints the *stored* document, so a draft holding unsaved edits would print
  without them.

Verified. `cargo fmt`; `SQLX_OFFLINE=true cargo clippy -p alo-store -p alo-jmap
--all-targets` clean; `cargo test -p alo-store -p alo-jmap` green, **50 new
tests** (11 IBAN, 8 settings-store units, 3 settings tenancy, 15 render, 6 HTTP,
7 web).
Web: `npx tsc --noEmit`, `npx eslint . --max-warnings 0`, `npm run test` (**151
in 21 files**) and `npm run build` all clean, for the workspace **and** for
`ALO_PRODUCT=mail`, whose bundle was probed and carries no billing route, no
`printSheet` and no settings screen — only the shared string catalogue, as
before this item.

The wrong-tenant proof for the new table is sharper than the others, because
`billing_settings` is keyed by the tenant alone: there is no id to guess and no
`NotFound` to return. `billing_settings_tenancy.rs` asserts that a tenant which
has never saved reads its **own blanks even after a neighbour has filled the
table in**, that a neighbour's save never reaches back (checked on the IBAN
specifically), that the identity is the tenant's and not the user's, that a
save is a full replace at the store door, and that a tenant deletion purges the
row — read directly, not through the tenant predicate.

The same four routes are held by `products/mail/alo-jmap/tests/billing_print_http.rs`
(new, 6 tests through the real router over a real Postgres), which the first
draft of this item did not have — the cold review's blocking finding, and it
was right: the design note *published* the wrong-tenant `404` and nothing
proved it. What it now proves: every route `401`s without a token and `404`s on
an id that does not exist; the three response headers are what the design note
claims; **a printed page is the one place in billing where two records are
rendered into one document**, so the neighbour there holds a distinctive legal
name and IBAN and A's paper is searched for both (as is every refusal, which
must leak nothing it declined); the identity is created by its first save and
merged, never replaced, afterwards; a draft/void/credit note/quote each print
as what they are; and **twenty** user-controlled fields across all three
records are each carried onto the page escaped — a field silently not printed
fails that test too. (The neighbour's first IBAN in that suite was invented and
the store refused it, which is the validator doing its job; the fixture now
carries a real one.)

Wire-verified against the **local** backend (docker `alo-pg`, the debug
`alo-jmap` on `127.0.0.1:8080`, fresh tenant `wireb116`). All four new routes
`401` without a token. Abbreviated:

```
GET   /billing/settings           (never saved)     -> 200 stated:false, all blank
PATCH /billing/settings {}                          -> 422 "legal name must not be empty"
PATCH /billing/settings {…,"iban":"NL92ABNA…"}      -> 422 "the check digits of this IBAN
                                                            do not match; check for a typo"
PATCH /billing/settings {…,"bic":"ABNANL2"}         -> 422
PATCH /billing/settings {"vatId":…} (no country)    -> 422 "state the country before the
                                                            VAT id…"
PATCH /billing/settings (the real identity)         -> 200  nl→NL, "nl 8123.45.678.B01"→
                                    NL812345678B01, "nl91 abna 0417 1643 00"→NL91ABNA0417164300
PATCH /billing/settings {"city":"Rotterdam"}        -> 200  a merge: the IBAN survives
GET   /billing/invoices/{draft}/print               -> 200 text/html, banner Draft, no number
                                    CSP default-src 'none' · nosniff · no-store
POST  /billing/invoices/{id}/issue                  -> 200 INV-2026-00001
GET   /billing/invoices/{id}/print                  -> 200 "Invoice INV-2026-00001",
                                    "NL91 ABNA 0417 1643 00", "Payable by 2026-08-20",
                                    EUR 1 843.60 net · VAT 9% 6.62 · VAT 21% 371.70 ·
                                    EUR 2 221.92 — the server's figures, two rates
GET   /billing/invoices/{credit}/print              -> 200 "Credit note", "corrects invoice
                                    INV-2026-00001", and NO bank account (grep: 0)
GET   /billing/quotes/{sent}/print                  -> 200 "Quote QUO-2026-00001",
                                    "stands until 2026-09-05", no bank account, no "Due date"
GET   /billing/invoices/no-such-id/print            -> 404
GET   /billing/invoices/{id}/print?lang=xx | fr     -> 200, <html lang="en"> (falls back)
```

**The item's done-when — a correct one-page A4 — was checked in a real headless
browser**, not asserted: Google Chrome's own print path
(`--headless --print-to-pdf --no-pdf-header-footer`) over each captured page,
with the resulting PDF's page count and `/MediaBox` read back.

```
issued.html  pages=1 (/Count=[1])  box=595.0x841.9pt  A4=yes
draft.html   pages=1 (/Count=[1])  box=595.0x841.9pt  A4=yes
credit.html  pages=1 (/Count=[1])  box=595.0x841.9pt  A4=yes
quote.html   pages=1 (/Count=[1])  box=595.0x841.9pt  A4=yes
long.html    pages=2 (/Count=[2])  box=595.0x841.9pt  A4=yes   (24 lines — see below)
```

Cuts and flags:

- **A logo is a monogram placeholder** — up to two initials from the legal name,
  drawn in a rounded square. A real logo is a Drive file plus an upload surface
  plus an embedding decision (a PDF/A-3 attachment cannot reference an external
  image), which is its own item. A blank rectangle on every invoice would be
  worse than initials.
- **A 24-line document paginates to two A4 pages**, which is correct, and the
  totals block, the payment block and every individual line carry
  `page-break-inside: avoid` so none of them is ever split. What has **not**
  been exercised is a "page 1 of 2" footer or a repeated issuer block on later
  pages; the column headers do repeat (`thead`). Worth an item if long
  documents turn out to be common.
- **The printed document is English only.** `strings_for()` is the seam and
  falls back rather than refusing; fr/nl land with the rest of the wave's
  translations at B1.27. Its number and date *formats* are already language-
  keyed (`group_separator`, `decimal_separator`), so a translation is a table
  entry, not a code change.
- **Country codes print as codes** (`DE`), not names, and only cross-border.
  Names need a per-language table — B1.27, with the translations.
- **Nothing is emailed and no PDF is produced.** B1.17 turns this page into a
  PDF and B1.18 attaches it to a mail **draft**; the loop sends no mail.
- **No browser click-path was exercised** (unchanged from B1.13–B1.15: there is
  no interactive browser here). What stands in for it: 7 new component tests
  driving the real screens through the real router and client — including the
  print button really fetching the server's page and really handing it to a
  print dialog through a `srcdoc` frame — the full curl arc above, and the
  headless-Chrome measurements. A human click-path remains the open step.
- **Server refusals are English** (flagged at B1.13, still open, and now
  visible in one more place: the settings form).
- **`cargo fmt --all -- --check` is red on ten files this item does not
  touch** (alo-ai ×2, alo-identity ×1, and seven alo-jmap modules), all of it
  import-ordering drift from the edition-2024 style. `cargo fmt` on the changed
  crates swept them, and that churn was **reverted** so this commit stays the
  item: a formatting-only commit is the right way to clear it, and it is not
  this item's to make.
- **`platform/alo-store/tests/common/mod.rs` defaults to port 5433**, while the
  `alo-pg` container publishes **5432** — the store suites fail with
  `PoolTimedOut` unless `DATABASE_URL` is set. Noted for whoever writes the
  runbook; not changed here, since the default may be right for compose.
- **HUMAN ACTION (still open) — `/billing` is a new top-level route prefix.**
  Unchanged from B1.05/B1.10/B1.12/B1.13/B1.14/B1.15: the production Caddyfile
  must add it at the next deploy, or every billing XHR gets the SPA. This item
  adds no new prefix — all four routes live under `/billing`.

Next item: B1.17 (server-side PDF: the design decision between headless
chromium in a pinned container and a pure-Rust HTML-to-PDF path, recorded in
`billing.md` first, then `GET /billing/invoices/{id}/pdf`).

---

## 2026-08-07 — B1.17 the PDF, written by us

`GET /billing/invoices/{id}/pdf` answers a real PDF file for any invoice of the
tenant, draft or issued or credited.

**The decision the item asked for, recorded in `docs/design/billing.md` before
any code — and it is a rejection first.** Headless chromium renders our own
page perfectly; what ruled it out is what it costs to own. It is a **new engine
in the deployment** (a ~1 GB image and a browser process on the invoice path,
which by our own doctrine needs an ADR and which the loop may not add, since it
cannot touch `deploy/`); it puts a **second, unpinned layout engine** between
the document and the paper, so the file a customer receives would depend on
whichever chromium build was in the image; and its failure modes — sandbox,
fonts, zombie processes, memory — are a browser's, in the one place where a
failure means an invoice that cannot be sent.

**Chosen: a pure-Rust writer, and precisely *not* an HTML-to-PDF path.** We do
not parse our own HTML back. The PDF and the printed page are **two renderers
over one model**: both are handed the same `PrintDocument`, the same `Strings`
table and the same `amount`/`quantity`/`rate`/`date`/`document_heading`
formatters, so neither can invent a figure the other does not have. A general
HTML+CSS engine in Rust would be a project; laying out a document whose shape
we already know is a page of arithmetic.

Shipped:

- **`platform/alo-pdf`** (new crate, **no dependencies**) — a minimal PDF 1.7
  producer: `writer.rs` (objects, the cross-reference table, the trailer, the
  font dictionaries), `canvas.rs` (one page and the marks on it), `text.rs`
  (styles, alignment, wrapping), `font.rs`, `metrics.rs`, `encoding.rs`,
  `color.rs`. In `platform/` because a PDF is not a billing concept — Drive
  exports and Docs want the same writer. Callers work in **points from the
  top-left, y downwards**; the flip into PDF's own space happens once, in the
  canvas, so no layout ever computes `height - y`.
- **`alo-jmap/src/billing_pdf.rs`** — the layout, quoting the print
  stylesheet's own proportions (A4 margins, the 90/22/26/16/26 mm columns, the
  same rules and palette), plus pagination, the response, and the file name.
- **`billing_invoices.rs`** — a `Printable` struct now gathers the document and
  both parties once, and *both* routes render from it, so `/print` and `/pdf`
  cannot drift apart. `billing_print`'s formatters became `pub(crate)` and its
  heading became `document_heading` for the same reason.

Decisions worth recording, all as-built in `docs/design/billing.md`:

- **The width tables are read, not remembered.** A PDF has no layout engine: a
  producer that wants a money column to line up must know every character's
  width before it places it. Both faces' WinAnsi advances were extracted from a
  real Helvetica (`hmtx`/`cmap`, scaled from 2048 to 1000 units/em) and are
  shipped **exactly as measured**, not hand-patched towards Adobe's AFM where
  the two differ on a handful of symbols (`€`, `±`, `÷`, `µ`) — a table that is
  "the measurement, except three values somebody remembered" is the one nobody
  can check. For everything a document prints, the two agree exactly. The same
  numbers are declared in the font dictionary, so a reader is told what we
  measured.
- **The file is an attachment, never inline**, and carries no CSP. A PDF
  rendered inline is rendered by a viewer inside our own origin; `attachment`
  closes that path, which is also the only path a policy would have bound.
- **`created` is passed in, not read.** The renderer never touches a clock, so
  one document renders to the same bytes every time — which is what makes a
  golden test possible and a re-download identical to the file the customer
  already holds.
- **Page numbers are stamped at the end**, once the total is known: `1 / 3` is
  the difference between a customer who can tell a page is missing and one who
  cannot. A one-page document says nothing.
- **A row never straddles a page, and a continuation page repeats the column
  headings** — a column of figures with nothing above it is one a reader has to
  guess at.

Verified. `SQLX_OFFLINE=true cargo clippy -p alo-pdf -p alo-store -p alo-jmap
--all-targets` clean (zero warnings); `cargo test -p alo-pdf -p alo-jmap` fully
green — **64 new tests**: 41 in `alo-pdf` (the cross-reference offsets checked
by reading the table back and confirming each points at the object it claims;
the declared stream length against the real one; determinism; the WinAnsi
folding, one real name per member state the fonts cannot spell; wrapping,
including a grouped amount that must never split and a column narrower than one
character that must still terminate), 16 over the layout, and 7 through the
real router. `rustfmt --edition 2024` clean on every file this item touched;
the pre-existing divergence in the inbox above is untouched.

**Every rendering assertion goes through an independent parser.** The tests do
not grep our own content stream — they run `pdf-extract` (already in-tree for
Drive search, now a dev-dependency here) over the bytes and assert on the words
it reads back, so what is checked is what a *reader* sees, decoding and all.

The **wrong-tenant proof** is `tests/billing_pdf_http.rs`: B holds a
distinctive legal name and IBAN; A gets `404` — never `403`, which would
confirm the id exists — for B's issued document *and* B's draft, no file is
served, and the refusal body is searched for all four of B's secrets. A ghost
id gets a byte-identical answer, so the status is not an existence oracle.
Then the sharper half, which only a printed document raises: A's *own* file is
rendered after B has filled the settings table in, and A's paper still prints
A's blanks — the whole file, metadata included, is searched for B's name and
account.

Wire-verified with curl against the local debug `alo-jmap` on `127.0.0.1:8080`
over docker `alo-pg` (fresh tenants `wireb117` and `wireb117b`):

```
GET  /billing/invoices/{id}/pdf   (no token)          -> 401
GET  /billing/invoices/no-such-id/pdf                 -> 404
GET  /billing/invoices/{draft}/pdf                    -> 200  8 506 bytes
        content-type: application/pdf
        content-disposition: attachment; filename="Invoice.pdf"
        x-content-type-options: nosniff · cache-control: no-store
POST /billing/invoices/{id}/issue                     -> INV-2026-00001
GET  /billing/invoices/{issued}/pdf                   -> 200  8 475 bytes
        content-disposition: attachment; filename="Invoice-INV-2026-00001.pdf"
        magic %PDF-1.7 … trailer %%EOF
GET  /billing/invoices/{credit}/pdf                   -> 200  7 997 bytes
        filename="Credit-note-INV-2026-00002.pdf", bank account absent (grep: 0)
GET  /billing/invoices/{A's invoice}/pdf   as tenant B -> 404 "no such invoice"
GET  /billing/invoices/ghost/pdf           as tenant B -> 404 (byte-identical)
```

The served files were then **read back with poppler**, not asserted:
`pdfinfo` reports `Pages: 1`, `595.276 x 841.89 pts (A4)`, `PDF version: 1.7`,
`Title: Invoice INV-2026-00001`, `Producer: alo workplace`; `pdftotext -layout`
shows the whole document — both parties, `12.5 hour × 120.00 = 1 500.00`,
`VAT 9% EUR 8.10`, `VAT 21% EUR 315.00`, `Total EUR 1 913.10`, `Payable by
2026-08-21`, `NL91 ABNA 0417 1643 00` — and `pdftoppm` renderings of the issued
invoice, a draft, a credit note and a 30-line document were looked at as images.
That last step earned its place twice: it caught the description column running
under the quantity beside it (the description was given the full width left of
the *quantity* column instead of the width of its own cell), which no assertion
in the suite would have noticed, and it confirmed the repeated headings and the
`2 / 3` footer on a continuation page.

Cuts and flags:

- **FLAGGED FOR HUMAN REVIEW — the PDF's character repertoire, and the font
  decision behind it.** The standard-14 fonts are the only fonts we have
  without shipping a font file, and they address WinAnsi (cp1252): Western
  Europe exactly, and **not** Polish, Czech, Slovak, Hungarian, Romanian, the
  Baltic languages, Greek or Cyrillic. Rather than print `?ukasz`, the encoder
  folds a letter to its base Latin form (`Łukasz` → `Lukasz`, `Ștefan` →
  `Stefan`); a non-Latin script is `?`. That is a lossy rendering of somebody's
  name on a legal document and it is recorded as such, not buried. **It ends at
  B1.22**: PDF/A-3, which Factur-X requires, forbids non-embedded fonts, so a
  font file lands there and takes the limitation with it. **Which** font —
  brand, licence, the weight of a binary in a public repository — is a human
  decision, which is why the loop did not download one. The HTML print view is
  unaffected and prints every one of those letters correctly.
- **No web UI.** The item's done-when is the endpoint and a curl transcript, so
  nothing in `web/` was touched and no gate there was run. A "Download PDF"
  button beside the existing **Print** button is a small follow-on; B1.18's
  **Send** is the surface that actually puts the file in front of a customer.
- **No quote PDF.** `/billing/quotes/{id}/pdf` is not in this item, and the
  queue is not a place to invent scope — but the renderer already lays a quote
  out correctly (it must: a quote is the same `PrintDocument`), and there is a
  unit test pinning that, so the route is a dozen lines whenever the queue asks.
- **The content streams are uncompressed.** An invoice is a few kilobytes
  either way, and a file a human can read in an editor is a file whose bugs can
  be seen. `FlateDecode` is additive whenever size matters.
- **Not PDF/A yet, deliberately.** No embedded font, no XMP metadata, no output
  intent, no attached CII invoice — all four are B1.22's, and all four are
  additions to this writer rather than changes to it.
- **The em dash, the minus sign and the narrow no-break space** our own
  formatters emit are folded to the WinAnsi characters nearest them (`–`/`—`
  survive; `−` becomes `-`, the narrow space becomes a no-break space). Pinned
  by tests so a later reading cannot quietly change what an amount looks like.
- **HUMAN ACTION (still open) — `/billing` is a new top-level route prefix.**
  Unchanged from B1.05 onwards: the production Caddyfile must add it at the next
  deploy, or every billing request gets the SPA. This item adds no new prefix.

Next item: B1.18 (send an invoice: `POST /billing/invoices/{id}/send` drafts an
email to the customer with this PDF attached, landing in Drafts for review —
the loop never sends mail).

---

## B1.18 — send an invoice: the email alo writes and never sends

`POST /billing/invoices/{id}/send` renders the invoice PDF, composes a short
covering note to the customer, and leaves the message in the user's **Drafts**
with `$draft`. It does not send. Sending stays the ordinary submission path
that the user triggers — the one path that DKIM-signs, records and is audited —
so billing gains no second way onto the wire. It is the rule the agent's draft
tools already follow (ADR 0034), and it is why the loop could build this item
at full depth without ever touching the safety rail against sending mail.

Three things are the server's, because a request must not be able to choose
where an invoice goes:

- **the recipient** — the customer's stored invoice address; there is no `to`
  field on the route at all;
- **the author** — the caller's own canonical address, read from the store;
- **the attachment** — rendered here, now, from the stored document; never
  uploaded, never a client-supplied blob id.

The only caller input is `?lang=`, which picks the words of both the note and
the document, exactly as `/print` and `/pdf` already do.

The refusals come from the document's own state, not from a flag: a **draft**
carries no number and prints a DRAFT banner, and a **void** invoice has been
cancelled — both `409`, each naming its state, because "409" alone leaves a
user guessing whether to issue this document or raise a new one. **Issued and
paid** may both be sent: re-sending a paid invoice as a copy for the customer's
records is a normal thing to do. A customer with no email address is a `422`
saying exactly that. Sending twice writes two drafts and changes no billing
record — the invoice has no "sent" state, and the mailbox is the record of what
was sent.

**What was extracted rather than duplicated.** The draft machinery lived inside
`agent.rs` as three private functions. Billing needed the same three, and a
billing route reaching into the AI agent module for them would have been the
dependency pointing the wrong way, so they moved to a new
`products/mail/alo-jmap/src/drafts.rs` — get-or-create a role mailbox, resolve
the caller's own send address, save an outgoing message into Drafts with
`$draft`. `agent.rs` now calls it and is 60 lines shorter; B1.26's dunning
reminders will call the same three. The rule that made the extraction worth
doing is written where the functions now are: **the author is resolved
server-side**, so nothing that composes a message — model, route or client —
can choose who it is from.

New code: `billing_send.rs` (route, the covering-note string table, the state
gate, the recipient check), `drafts.rs` (the shared machinery),
`tests/billing_send_http.rs`. `billing_invoices.rs` opened `Printable` to the
crate so the note, the page and the file are all rendered from one read of the
document rather than three.

Verified — `cargo clippy -p alo-jmap --all-targets` clean, `cargo test -p
alo-jmap` green (149 unit + every integration suite, 7 new), then on the wire
against the debug `alo-jmap` on `127.0.0.1:8080` over docker `alo-pg` (fresh
tenants `wireb118` and `wireb118b`):

```
POST /billing/invoices/{id}/send   (no token)            -> 401
POST /billing/invoices/no-such-id/send                   -> 404 "no such invoice"
POST /billing/invoices/{draft}/send                      -> 409 "a draft is not sent
                                                            to a customer — issue it first"
POST /billing/invoices/{id}/issue                        -> INV-2026-00001, due 2026-08-21,
                                                            gross 191310 cents
POST /billing/invoices/{issued}/send                     -> 200
     {"draft":{"id":"VCZ9jcPtO9-0Ukc_ciMbow",
               "to":"buchhaltung@kunde.test",
               "subject":"Invoice INV-2026-00001 — Alo Werkplaats B.V.",
               "attachment":{"name":"Invoice-INV-2026-00001.pdf","sizeBytes":8258}}}
POST /billing/invoices/{no-email customer}/send          -> 422 "this customer has no
                                                                 email address"
POST /billing/invoices/{void}/send                       -> 409 "a void invoice is not sent"
POST /billing/invoices/{A's invoice}/send  as tenant B   -> 404 (byte-identical to a ghost id)
```

Then the draft was read back **as a mail client reads it**, not asserted:
`Mailbox/get` shows `Drafts role=drafts total=1` and **no Sent folder at all**
— nothing was sent, and nothing created the folder that would say otherwise.
`Email/get` returns `keywords:{"$draft":true}`, `to:[{"email":
"buchhaltung@kunde.test","name":"Kunde & Söhne GmbH"}]`, `from` the caller's
own address, the subject above, `hasAttachment:true`, and with
`fetchTextBodyValues` the note itself —

```
Dear Kunde & Söhne GmbH,

Please find attached Invoice INV-2026-00001 for EUR 1 913.10, payable by 2026-08-21.
```

— beside one attachment, `application/pdf`, `Invoice-INV-2026-00001.pdf`, 8258
bytes. `GET /jmap/download/{acc}/{blob}~a0/…` served those bytes: `%PDF-1.7`,
`pdfinfo` reading `Title: Invoice INV-2026-00001 · Producer: alo workplace`,
and `pdftotext -layout` showing both parties, both VAT rates and the total. The
same download **as tenant B is a 404**. The figure in the note and the figure
on the page are the same one because both come from the document's own
formatter — asserted in the suite, and true on the wire here.

Cuts and flags:

- **No web UI.** The item's done-when is the draft existing on the wire, and it
  does. A "Send to customer" button beside **Print** and **Download PDF** is a
  small follow-on and belongs with the other billing UI work; nothing in
  `web/` was touched, so no gate there was run.
- **No `sentAt` on the invoice.** Recording that a document was sent would be a
  migration and a new field, and the queue does not ask for one here. The
  mailbox is the record: the draft, and then the sent copy, are both in it.
  Worth a human's decision before B1.26 (dunning) invents its own answer.
- **The note is English-only**, on the same seam as the document's strings
  (`mail_strings_for`, pinned by a test): fr/nl land together at B1.27 without
  touching a caller.
- **A quote is not sent.** `POST /billing/quotes/{id}/send` already exists and
  means something else entirely — a lifecycle transition, no mail. The two are
  a genuine naming collision in the contract; the route module and the design
  note both say so in as many words, and renaming either would be a breaking
  change to a public surface for a cosmetic gain. Flagged, not fixed.
- **The covering note is plain text.** No HTML alternative: the document is the
  attachment, and a plain note is the one that renders identically everywhere.
- **HUMAN ACTION (still open) — `/billing` is a new top-level route prefix.**
  Unchanged since B1.05: the production Caddyfile must add it at the next
  deploy, or every billing request gets the SPA. This item adds no new prefix.

Next item: B1.19 (payments: `billing_payments`, partial payments, the derived
paid / partially-paid state, and the overdue view).
