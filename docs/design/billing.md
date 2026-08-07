# Design note — alo Billing (customers, products, quotes, invoices, payments)

Status: building · 2026-08 · ADR 0035 · Business track wave B1

alo Billing is the first Work OS module: the quote → invoice → payment
arc for EU SMEs, with legal sequential numbering and EN 16931
e-invoicing as the wedge. It is built from scratch on the tenant-scoped
store, money is integer cents everywhere, and every total is computed
server-side. This note records the surface, data model, error map,
tenancy, and the numbering decision before the first migration lands;
it is updated to as-built at the B1 wave review (B1.27).

## Surface

- **Inputs:** authenticated workspace users driving `/billing/*` routes
  on `alo-jmap` — customer and product CRUD, quote and invoice draft
  CRUD, the issue and credit-note actions, payment recording, the VAT
  report, and the PDF/e-invoice renderings. The billing agent
  (ADR 0034, item B1.25) is a second caller of the same store
  functions, never of a parallel code path.
- **Outputs:** JSON resources with server-computed totals; a printable
  HTML document and its PDF rendering; Factur-X (CII) and XRechnung
  (UBL 2.1) XML for issued invoices; CSV for the VAT summary; and mail
  **drafts** (never sends) when an invoice or reminder goes to a
  customer.
- **Who calls it:** `web/src/billing` (the module UI, B1.13–B1.16)
  calls `alo-jmap`; the `alo-ai` billing module produces
  propose-then-approve envelopes that `alo-jmap` executes; nothing
  external calls billing directly in B1 (Peppol is a later item and
  goes through a certified access point, not our own endpoint).

### Web surface — as-built (B1.13)

`web/src/billing` is a rail module of the **workspace product only**
(`product/workplace.tsx`), mounted at `/billing/*` with a tab per record
type: `customers` and `products`, and `/billing` redirecting to the
first. Later items add tabs (invoices, quotes), never a second
navigation idea.

Three rules the module holds itself to, so the screens can never become
a second, weaker definition of billing:

- **No validation in the client.** A form sends what was typed; a `422`
  is shown in the server's own words next to the form, which stays open
  and keeps everything the user entered. The only client-side refusal is
  text that is not a number at all (`money.ts`), because turning typing
  into integer cents is inherently the client's job.
- **No money is computed in the browser.** `money.ts` parses one typed
  decimal into hundredths (cents, or basis points for a rate) and
  formats one back; every total comes from the API.
- **An edit sends only the fields that changed.** The surface is
  last-writer-wins (no `ETag` yet), so a field nobody touched is not
  written. A cleared text box sends `null`, which is how a VAT id comes
  off a customer.

`web/src/billing/api.ts` is a small client of its own rather than more
methods on `JmapClient`: billing is plain REST, with none of JMAP's
session or method-call envelope, and it changes for different reasons.
It shares the auth layer's `authorizedFetch`, so there is one session.

The tabs as built are `invoices` (also what `/billing` lands on),
`quotes`, `customers`, `products`.

### Documents on screen — as-built (B1.14, B1.15)

An invoice and a quote are the same document with different words on
it, so they are **one screen**, not two that look alike:
`documentDraft.ts` holds the form and the autosave loop,
`DocumentEditor.tsx` renders it, `DocumentLines.tsx` is the line grid,
and `InvoiceEditor` / `QuoteEditor` supply only what differs — the
words, the two dates, the state chips, and the transitions.

Three rules on top of the module's own:

- **A transition acts on the stored document, so it waits for the
  form.** While the draft holds edits the server has not stored, every
  lifecycle button is disabled and says why. Firing one then would
  freeze a document that is not the one on screen, and the keystrokes
  since the last save would be lost inside a document nobody can edit
  again. A row that cannot become a line keeps this true indefinitely,
  which is correct.
- **Every transition asks first, and the dialog states what it does to
  the document** — spends the next number of the series and freezes it,
  closes the offer for good — rather than asking whether the user is
  sure. Each is irreversible on a legal document.
- **A transition request carries no body.** What a document becomes is
  the route, never a field a stale form could have sent.

Where a transition answers with a *different* document — accepting a
quote, raising a credit note — the screen goes to that document, because
it is the one that now needs work. Both directions of each link are on
the record: an invoice names the quote it came from and the invoice it
credits; a quote names the invoice it became.

### The printed document — the decision (B1.16)

**Chosen: the printable document is rendered on the server**, by
`alo-jmap`, as one self-contained HTML page (`billing_print.rs`) —
inline CSS, no script, no external asset of any kind.

**Rejected: rendering it in the browser from the React module.** Three
things make the client the wrong place for it:

- **It is the PDF source** (B1.17). Whatever produces the PDF —
  headless chromium or a Rust HTML-to-PDF path — runs *without a
  browser session*, so a client-rendered document would have to be
  reimplemented server-side, and the paper the customer holds would
  come from a second, drifting definition of the same document.
- **It is also the mail attachment** (B1.18), produced when nobody is
  looking at a screen at all.
- **It must be printable from a page we do not style.** A document
  assembled from the app's `ds` tokens inherits the app's layout; a
  standalone page with its own `@page` rules is what actually yields
  an A4 sheet.

The browser therefore *fetches* the document rather than composing it:
`GET /billing/{invoices,quotes}/{id}/print` with the session's bearer
token, the returned HTML into a hidden `srcdoc` iframe, `print()`. A
plain link would open an unauthenticated tab, and printing a document
is not a reason to invent a second way in.

Rules the renderer holds itself to:

- **Every value is escaped, and the page can reach nothing.** Customer
  data goes through one escaper, so a defect there is the only way
  markup could appear at all — and two *different* mechanisms stop it
  becoming a request, one per place the page is used. Fetched as a
  document (headless chromium at B1.17, a saved file, a mail client)
  the response's own `Content-Security-Policy: default-src 'none'`
  binds it. Mounted by the web app it is copied into a **same-origin
  `srcdoc` frame**, which inherits the *app's* policy and never sees
  that header — so the frame is **sandboxed without `allow-scripts`**.
  Neither mechanism substitutes for the other, and the code says so in
  both places.
- **The document says what it is.** A draft prints as a draft and
  carries no number (it has none); a void invoice prints as void; a
  credit note is titled as one. A printed page that could be mistaken
  for an issued invoice is a legal problem, not a cosmetic one.
- **No money is computed here either.** The renderer prints the store's
  cents; it only groups digits.
- **Its words are a table, not literals in the markup**
  (`billing_print::Strings`), keyed by document language — the same
  externalisation rule as the web catalogues, in the one place a
  customer-facing string is emitted by Rust. `en` ships now; fr/nl at
  the wave review (B1.27). An unknown language falls back to the
  default rather than refusing: a filter may be strict, but a document
  that will not print because of a display preference is worse than a
  document printed in English.

**The issuer's own details** — who is billing, their VAT and
registration numbers, and the bank the money goes to — are a *tenant*
record, not a per-document one, so B1.16 also lands `billing_settings`
(below). The logo is a **monogram placeholder** drawn from the legal
name: a real logo is a Drive file and an upload surface, which is its
own item, and a blank rectangle on every invoice is worse than initials.

As-built (B1.16), the decisions the page itself forced:

- **Dates print as ISO `YYYY-MM-DD` in every language.** `05/03/2026`
  is two different days depending on who reads it, and a due date a
  customer can misread by two months is a dispute. EN 16931 dates are
  ISO for the same reason.
- **Amounts are grouped and carry the ISO currency code**
  (`EUR 1 843.60`), never a symbol: the code is what the e-invoice
  schemas want and is unambiguous across member states.
- **The number is stated once.** It is in the heading, so the grid
  beside it does not repeat it — a document that states its own number
  twice makes a reader check whether the two agree.
- **A domestic address does not print its country.** Postal convention
  names the country only when the document crosses a border, and a lone
  `NL` under a Dutch address reads like a stray field. Cross-border it
  is the line that decides the VAT treatment, so it stays. (Country
  *names* rather than codes need a table per language — B1.27.)
- **A quote and a credit note print no bank details.** Both say
  explicitly that nothing is payable; an IBAN under that sentence is
  how a document gets paid twice. An invoice with no due date yet (a
  draft) states the term instead, so the page never simply omits when
  the money is owed.
- **The issuer is read live, not snapshotted at issue.** Reprinting last
  year's invoice shows the current address and bank, which is what
  moving office or changing bank is supposed to do; the facts that must
  never drift — number, dates, lines, money — are on the document.
- **`?lang=` falls back rather than refusing**, unlike the `status`
  filter: a filter that silently widened would mislead a bookkeeper, but
  a document that will not print because of a display preference is
  worse than one printed in English.

### Routes

All under the authenticated `alo-jmap` router, following the existing
action-route convention (typed `Problem` errors, `authenticate`
extractor, registered in `server.rs`):

| Route | Purpose |
|---|---|
| `GET/POST /billing/customers`, `GET/PATCH/POST /billing/customers/{id}[/archive]` | customer CRUD (B1.05) |
| `GET/POST /billing/products`, `GET/PATCH/POST /billing/products/{id}[/archive]` | price-list CRUD (B1.05) |
| `GET/POST /billing/invoices`, `GET/PATCH/DELETE /billing/invoices/{id}` | draft CRUD + list with status filter (B1.10); `DELETE` is draft-only, an issued document is voided instead |
| `POST /billing/invoices/{id}/issue` | assign number, freeze (B1.10) |
| `POST /billing/invoices/{id}/void` | cancel an issued document, keeping its number (B1.10) |
| `POST /billing/invoices/{id}/credit-note` | create the crediting invoice (B1.10) |
| `GET /billing/settings`, `PATCH /billing/settings` | the issuer's own identity and bank details (B1.16) — **as built** |
| `GET /billing/invoices/{id}/print[?lang=]`, `GET /billing/quotes/{id}/print[?lang=]` | the printable document as one self-contained HTML page (B1.16) — **as built** |

As-built (B1.10), for the invoice routes specifically:

- **The header and the line set travel in one body.** `lines` is an ordinary
  field of the invoice body on both `POST` and `PATCH`, replacing the whole set
  in the order sent; absent, it leaves the stored lines alone. A draft editor
  saves the document it is looking at, not a patch stream. A body that states
  only `lines` deliberately does **not** touch the header — replaying the stored
  header would re-resolve the customer, and a draft whose customer was archived
  afterwards could then never have its lines edited again.
- **Money is only ever read.** Every response carries server-computed `totals`
  (net, gross, and the VAT breakdown per rate) and a per-line `netCents`; there
  is no writable total anywhere in the surface. There is no per-line VAT field,
  because VAT is rounded once per rate subtotal and a per-line column would not
  add up to the document's own.
- **`overdue` is derived on read** (`Invoice::is_overdue`) from the status and
  the frozen due date, never stored — a stored flag would be wrong every
  midnight — and judged against the server's date, never one a client sends.
- **The `status` filter is strict** (`422` on an unrecognised value), unlike the
  forgiving boolean query flags: a filter that silently widened to "everything"
  would show a bookkeeper drafts among their issued documents.
- **`GET /billing/invoices/{id}` also answers `creditNotes`** — the summaries of
  what credits this document, drafts included: the ledger of a corrected
  invoice, and the read the issued view needs.
- **Lifecycle transitions are their own `POST`s**, never fields on the `PATCH`,
  so issuing (which assigns a legal number and freezes the document) can never
  happen because an editor submitted a stale form.
- **`status`, `number`, `issueDate` and `dueDate` are not writable** by any
  request; like any unknown field they are ignored.

The rest of the surface, **not yet built** (the wave item that lands each is
named):

| Route | Purpose |
|---|---|
| `GET /billing/invoices/{id}/pdf`, `.../xrechnung.xml` | renderings (B1.17, B1.23) |
| `POST /billing/invoices/{id}/send` | draft an email with the PDF attached (B1.18) |
| `GET/POST/PATCH/DELETE /billing/quotes[/{id}]`, `POST .../{send,accept,decline,expire}` | quote lifecycle, and accept → draft invoice (B1.11, B1.12) — **as built** |
| `GET/POST /billing/payments` | record full/partial payments (B1.19) |
| `GET /billing/reports/vat?from&to` | VAT summary + CSV (B1.20) |

`/billing` is a **new top-level route prefix**: the production Caddyfile
needs it added at the next deploy. That is a human action recorded in
`docs/autonomy/STATE.md`, never a change the loop makes to `deploy/`.

## Data model

New `billing_*` store modules in `platform/alo-store` (one file per
responsibility, mirroring `tasks.rs` / `calendar.rs`). Every table
carries `tenant_id`, ids are `opaque_id!` newtypes, timestamps are
`timestamptz`, and **money is `i64` integer cents** with VAT rates in
**basis points** (`i32`, 2100 = 21 %). No floating point appears in any
column, struct field, or computation anywhere in this module.

- **`billing_customers`** — display name, address lines, postal code,
  city, country (ISO 3166-1 alpha-2), VAT id (nullable — B2C customers
  have none), email, payment terms in days, default currency
  (ISO 4217), optional link to an existing `contacts` row, archived
  flag. Archive rather than delete: an issued invoice must always be
  able to name its customer.
- **`billing_products`** — name, unit, unit price cents, VAT rate bp,
  archived flag (as-built: an `archived_at` timestamp, the same shape as
  `billing_customers`, so the `/archive` route and the pickers behave
  identically across the module). Prices are in the tenant's default
  currency; the document carries the currency it was raised in, and
  B1.21's FX snapshot converts. A price-list *source* for lines, not a
  foreign key the line depends on — see the line snapshot rule below.
- **`billing_invoices`** — customer ref, status
  `draft | issued | paid | void`, currency, optional number (NULL while
  draft), issue date, due date, payment terms snapshot, credit-note
  type flag with a nullable `credits_invoice_id` self-reference, and
  the stored FX rate snapshot for multi-currency (B1.21).
  As-built (B1.06): also `reference` (the customer's PO number) and
  `note`, both printed on the document; the currency and terms are
  snapshotted from the customer when the draft is raised, and a new
  document cannot be raised for an **archived** customer. The FX column
  is not in the table yet — it arrives, additively, with B1.21. The
  status/number/date invariants are enforced by CHECK constraints as
  well as in Rust: a draft is exactly a document with no number and no
  dates, so an abandoned draft can never consume a number, and
  `(tenant_id, number)` is unique.
  As-built (B1.07): the draft-only rule is enforced on **every** write —
  header, lines and deletion — by re-reading the status under the row's
  `FOR UPDATE` lock inside the same transaction that writes, so an edit
  that raced an issue is refused (`Conflict`) rather than applied to a
  numbered document. The state refusal outranks any complaint about the
  payload, and deletion is draft-only: an issued document is voided,
  keeping its number so the sequence stays gapless.
  As-built (B1.09): a **credit note is an invoice in this same table**, not a
  second document type — it names its original in `credits_invoice_id`, carries
  that document's lines with their quantities negated, and goes through the same
  draft → issued life, drawing from the same series. Raising one copies the
  original's customer, currency, terms and customer reference (never the note:
  the original's "payable within 14 days" says the opposite of the truth on a
  credit note) and leaves it a **draft**, so a *partial* credit is simply a
  matter of editing its lines before issuing. The customer is copied rather than
  re-resolved, so an **archived** customer can still be credited — correcting a
  document already in their hands is not new business. While it is a draft a
  credit note is editable like any other, except that its customer and currency
  are pinned to the original's (`Validation`, `422`): a credit billed to
  somebody else reverses nothing. Only an `issued` or `paid` document can be
  credited; a draft (`Conflict`, `409` — delete it instead) and a void one
  (already cancelled in full) cannot, and neither can a credit note itself —
  that refusal is about what the document *is*, so it outranks and does not
  vary with its status. `GET`-side, `billing_credit_notes(original)` lists what
  credits a document, which is the ledger of a corrected invoice.
- **`billing_invoice_lines`** — invoice ref, line order, description,
  quantity in **milli-units** (`i64`; 1.5 h = 1500), unit price cents,
  VAT rate bp. Lines **snapshot** the product's description, price, and
  rate at the moment they are added; later edits to the price list
  never rewrite an existing document. This is the whole reason a line
  does not simply join to `billing_products`.
  As-built (B1.06): a document's lines are written as a **whole set** in
  one transaction, in the caller's order (`line_order` is 0-based and
  contiguous) — a draft editor sends the document it wants rather than a
  patch stream, so there is no half-edited state and no ambiguity about
  order. A negative quantity is legitimate (that is how a discount line
  is written); a negative unit price is not. The bounds — |qty| ≤ 10^9
  milli-units, price ≤ 10^9 cents, ≤ 500 lines — are what make the
  totals arithmetic provably `i64`-safe.
- **`billing_quotes`** + **`billing_quote_lines`** — the same line
  model (shared code where it stays clean, not a forced abstraction),
  lifecycle `draft | sent | accepted | declined | expired`, valid-until
  date, and a link from the invoice created on acceptance back to the
  quote.
  As-built (B1.11): the sharing is literal — `billing_line.rs` owns the
  line model, the field rules, the read, and the single `INSERT` both
  document types write through (`LineTable`, differing only in the table
  and the column naming the document). What is *not* shared is the life:
  a quote is its own table because an invoice is owed money under a
  legally gapless number while a quote is an offer that can simply be
  turned down, and folding them together would put a quote's states
  inside the CHECK that guards invoice numbering.
  **The lifecycle is one pure transition table** (`QuoteStatus::
  allowed_next`): `draft → sent`, `sent → accepted | declined |
  expired`, and nothing else — unit-tested over all twenty-five ordered
  pairs, including every self-transition, which is refused (re-sending
  would draw a second number). The three closing states are
  **terminal**: a change of mind is a new quote, so the document the
  customer holds and the record of what they were offered stay the same
  thing. A refusal names both states *and* what the current one does
  allow, so a UI corrects itself without a second round trip.
  **Sending** is the quote's issue: it draws from a `quote` series
  (`QUO-YYYY-NNNNN`, kind `quote` in `billing_sequences`) — deliberately
  not the invoice series, since an unaccepted offer must not leave a
  visible hole in invoice numbering — stamps `sent_date` from the
  database's own `CURRENT_DATE` inside the transaction, derives
  `valid_until` from the `valid_days` snapshotted on the document
  (default 30, range 0–365), and freezes the content. A quote with no
  lines cannot be sent (`Validation`, `422`), exactly as an empty
  invoice cannot be issued.
  **Expiry is a fact and a decision.** `Quote::is_expired(today)` is
  derived on every read like an invoice's overdue flag; moving the quote
  to `expired` is a separate recorded act with a `decided_date`. There
  is deliberately **no background sweep**, and acceptance refuses on
  *state*, never on a date — honouring a lapsed offer a few days late is
  a decision the tenant is entitled to make.
  As-built (B1.12): **accepting an offer and raising the invoice for it
  are one act**, in one transaction under the quote's row lock, and
  `accept_billing_quote` answers with both (`QuoteAcceptance`). Either
  the offer closes and its draft invoice exists or nothing happened: an
  accepted quote with nothing to bill it by would be unrepairable,
  because acceptance is terminal and no retry could finish the job. The
  link is `billing_invoices.quote_id` (migration 0106) — on the newer
  document, which knows its own origin, rather than on a quote that is
  frozen the moment it is sent — with a composite foreign key to the
  same tenant and a **unique** partial index: one invoice per accepted
  offer, ever, so "the invoice raised from this quote" is a single row.
  A credit note may never carry one (CHECK), since it credits an
  invoice, not an offer.
  What the invoice copies: the customer, the currency and the customer's
  reference, plus every line unchanged at the price it was offered at,
  in the offer's order (`Line::copied`) — so the totals agree to the
  cent, including the VAT breakdown per rate. What it does not: the
  **note** (a quote's note states the terms of an *offer*, which is
  untrue of a bill) and the **payment terms**, which a quote does not
  carry and are taken from the customer as any new invoice's are. The
  customer is copied, not re-resolved, so an offer to a customer
  archived since it was sent can still be honoured — as a credit note
  can still be raised for one — while raising a *new* quote for them
  stays refused. The invoice is a **draft**: what was offered is what
  will be billed, but when, and whether in one go, is the tenant's
  decision, and the legal number comes only from `/issue`.
- **`billing_payments`** — invoice ref, date, amount cents, method,
  reference. Invoice paid-state is **derived** from the sum of its
  payments against its gross total, never stored as an independently
  writable field that could disagree with the ledger.
- **`billing_sequences`** — `(tenant_id, kind, year, next_value)`, the
  row-locked counter behind legal numbering (below).
- **`billing_settings`** (B1.16) — **one row per tenant**, the issuer
  side of every document: legal name, address, country, VAT id,
  company registration number, contact email/phone/website, and the
  bank the money goes to (IBAN, BIC, account holder). Tenant-wide, as
  customers and products are: a tenant issues under one identity.
  The row is created on first save; a tenant that has never saved reads
  the **blanks**, never a `404` — a print view asking "have you
  configured billing yet" would be a second source of truth about a
  record that always conceptually exists. The IBAN is held to its
  ISO 13616 length-per-country **and its mod-97 check** (`iban.rs`), the
  same standard the VAT id gets: a typo'd IBAN is money that never
  arrives, and it is caught at the point of entry or not at all.

### Totals

Totals are a **pure function** over lines (B1.06), never a stored
column the client can influence:

```
line_net   = round(qty_milli × unit_price_cents / 1000)
net        = Σ line_net
vat_by_rate[r] = round(Σ line_net where rate = r × r / 10_000)
gross      = net + Σ vat_by_rate
```

Rounding is at the **VAT-rate subtotal**, not per line — the EN 16931 /
VAT-directive convention (BR-CO-17: the category tax amount is the
category taxable amount times the rate) — and the property tests assert
that line sums always reconcile to the returned totals for randomly
generated documents. The client renders what the API returns; the web
layer never computes money (B1.14).

As-built (B1.06), one decision the first sketch left open: `round` is
half **away from zero**, not half up. The two agree on positive amounts;
they differ on negatives, and away-from-zero is what makes a credit note
the exact mirror of the document it credits — `totals(−lines) ==
−totals(lines)`, asserted as a property. Half-up would leave a one-cent
residue on any document whose credit rounds at a half, and a ledger that
does not sum to zero is an accounting defect, not a rounding taste.
Every intermediate is computed in `i128` and narrowed with saturation,
so the function is total for any input a future caller hands it.

## Errors

Store errors are `StoreError` variants (`thiserror`), mapped at the
route edge to the existing `Problem` shape. The full map:

| Condition | Store | HTTP |
|---|---|---|
| Unauthenticated request | — | `401` |
| Customer/product/invoice id absent **or owned by another tenant** | `NotFound` | `404` |
| Malformed VAT id for the customer's country | `Validation` | `422` |
| Negative quantity, negative unit price, unknown currency, VAT rate outside 0–10000 bp | `Validation` | `422` |
| Editing lines **or the header** of a non-draft invoice | `Conflict` | `409` |
| Deleting a non-draft invoice (it is voided, never deleted) | `Conflict` | `409` |
| Issuing an already-issued invoice | `Conflict` | `409` |
| Issuing an invoice with no lines | `Validation` | `422` |
| Voiding anything but an issued invoice | `Conflict` | `409` |
| Crediting a draft (never-issued) invoice | `Conflict` | `409` |
| Crediting a void invoice (already cancelled in full) | `Conflict` | `409` |
| Crediting a credit note | `Conflict` | `409` |
| Moving a credit note off its original's customer or currency | `Validation` | `422` |
| Creating an invoice without naming a customer (`customerId` absent or blank) | — (route edge) | `422` |
| Listing with a `status` filter that is not one of the four states | — (route edge) | `422` |
| Payment amount ≤ 0, or recorded against a draft | `Validation` | `422` |
| Invalid quote transition (e.g. `declined` → `accepted`) | `Conflict` | `409` |
| Editing, replacing the lines of, or deleting a non-draft quote | `Conflict` | `409` |
| Sending a quote with no lines | `Validation` | `422` |
| Quote validity outside 0–365 days | `Validation` | `422` |
| Accepting a quote that is not an open offer (draft, or already answered) | `Conflict` | `409` |
| Creating a quote without naming a customer (`customerId` absent or blank) | — (route edge) | `422` |
| Listing quotes with a `status` filter that is not one of the five states | — (route edge) | `422` |
| Saving billing settings without a legal name | `Validation` | `422` |
| Malformed issuer VAT id, IBAN (length or mod-97) or BIC | `Validation` | `422` |
| Printing a document that is absent **or another tenant's** | `NotFound` | `404` |
| Sequence row contention beyond the tx retry | `Db` | `503` |

The wrong-tenant case deliberately returns the **same `404`** as a
truly absent id: there is no existence oracle across tenants, matching
the `StoreError::NotFound` doctrine already documented in
`platform/alo-store/src/error.rs`.

## Tenancy

Every billing table carries `tenant_id`, and every read and write goes
through `Store::for_account(tenant, user)` — the `AccountStore` door
that bakes `(tenant, user)` into the query rather than accepting a
tenant argument a caller could get wrong. No billing function takes a
`TenantId` parameter; the handle is the scope.

Concretely:

- Every `SELECT`, `UPDATE`, and `DELETE` includes `tenant_id = $1` from
  the handle, never from request input.
- Foreign keys are validated **within the tenant**: attaching a
  customer to an invoice re-checks that the customer id resolves under
  the same handle, so a guessed id from another tenant is a `404`, not
  a cross-tenant link.
- The numbering sequence is keyed by tenant, so two tenants issuing
  concurrently never share a counter.
- **Every B1 storage item ships a wrong-tenant test** (mandatory per
  CLAUDE.md and LOOP.md): tenant A reaching tenant B's customer,
  product, invoice, quote, and payment each gets a clean denial —
  proven by a test, not asserted in prose.

## Numbering — the decision

**Chosen:** a per-tenant row in `billing_sequences`, selected
`FOR UPDATE` **inside the same transaction** that writes the invoice
number, issue date, and frozen status. Format `INV-YYYY-NNNNN`, the
counter resetting per year, credit notes drawing from the same sequence
so the ledger stays continuous.

**Rejected: a Postgres `SEQUENCE` / `nextval()`.** Sequences are
deliberately non-transactional — a rolled-back or failed transaction
**burns** the number it drew, leaving a permanent gap. Gapless
numbering is a legal requirement for invoices across the EU
(§14 UStG in DE, and the equivalent in FR/BE/NL), so the very property
that makes `nextval()` fast and contention-free is the property that
makes it unusable here. Row locking serialises issuance per tenant,
which is correct and cheap at SME volume; B1.08's concurrency test
asserts across 100 iterations that two parallel issues never share or
skip a number.

Drafts stay **unnumbered** (`number IS NULL`) precisely so an abandoned
draft cannot consume a number, and issuing is the only transition that
assigns one.

### As-built (B1.08)

- The row is `(tenant_id, kind, year) → next_value`, created on first
  use at 2 (handing out 1) by the same upsert that advances it, so a
  never-used series has no row at all. `kind` is shape-checked rather
  than list-checked: quotes (B1.11) add a row, never a migration.
  The upsert holds the counter's row lock until the issuing transaction
  ends, which is the `FOR UPDATE` this section promised, in one
  statement.
- **The issue date is the database's `CURRENT_DATE`, read inside the
  issuing transaction — not a caller-supplied date.** A series whose
  numbers ascend while their dates do not is not gapless in any sense a
  tax authority accepts, and backdating is how that happens. The due
  date is that day plus the terms already snapshotted on the document.
  *Flagged for human review:* bookkeepers do sometimes need to issue
  "as of" an earlier day (a month-end run done on the 3rd). Offering it
  needs a rule that keeps number order and date order together — its own
  queue item, not a quiet parameter.
- **An invoice with no lines cannot be issued** (`Validation`, `422`).
  It would be worth nothing and would spend a number of a legally
  unbroken series on a document that says nothing.
- **Voiding** is available only from `issued`: a draft is deleted (it
  took no number), a void document is already void, and a **paid** one
  is corrected with a credit note (B1.09), not cancelled by fiat. A
  voided document keeps its number, dates and lines — that is what keeps
  the series unbroken — and stays readable. Voiding suits a document
  that never left the building; one the customer already holds should be
  credited instead, so both parties' copies still reconcile. The store
  cannot tell those apart, so it allows the transition and says so.
- Issuing takes the **document's** lock before the **counter's**, in
  that order on every path, so concurrent issues queue instead of
  deadlocking, and a save that raced an issue is refused rather than
  landing on a numbered document.

## Out of scope for B1

Deliberate cuts, each a decision rather than an omission:

- **Payroll and tax filing** — excluded by ADR 0035; we export, we do
  not file.
- **Peppol network membership** — B1 sends via a certified access
  point; obtaining our own AP account is a human action logged in
  STATE.md, not loop work.
- **Live PSD2 bank feeds** — reconciliation arrives in B4 from imported
  statements (CAMT.053/MT940/CSV); no licensed aggregator in B1.
- **Payment links / PSP integration** and **recurring invoices** — both
  explicitly B2 in `docs/features.md`.
- **Customer self-service portal** — tagged `[B+]`, post-traction.
- **Automatic sending of any email** — B1.18 and B1.26 create Drafts
  the user approves, consistent with the ADR 0034 agent send rules and
  the loop's absolute no-real-email rail.
- **Live AI model calls in the loop** — the billing agent (B1.25) is
  verified structurally: routes exist, guards return 401/422, executors
  run against the local DB. Model wiring is a human step.
