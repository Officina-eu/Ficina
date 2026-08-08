# Design note — alo Finance (the books: expenses, the ledger, the bank, the return)

Status: **design** (B4.01, written before the first migration) · ADR 0035 ·
Business track wave B4

alo Finance is the fourth Work OS module and the first one whose output is
not a screen a colleague reads but a **statement a stranger audits**. B1
raises documents, B2 wins them, B3 fills them with hours; B4 answers the
question those three cannot: *what does this business actually own, owe and
earn* — and it must answer it the same way a tax inspector, an accountant
and a bank each expect, in a country whose rules alo does not get to choose.

The module is one sentence long, and every decision below is answerable by
asking which part of it is being protected: **every document that becomes
real posts a balanced journal entry the moment it becomes real, the bank
statement is matched against those entries by a human confirming a
suggestion, and the four reports are nothing but that journal, added up.**

Two consequences of that sentence are worth stating before the detail,
because they are what make this wave different from the three before it:

- **The ledger is not a view.** It is stored, append-only, and never edited.
  A wrong entry is corrected by a reversal, exactly as a wrong invoice is
  corrected by a credit note (B1.09) — the same discipline, one layer down.
- **The ledger never invents a figure.** Every cent it posts comes from a
  document that already exists and was already checked: `billing_totals`
  computed it, `billing_fx` crossed it, `billing_einvoice_import` refused it
  if it did not add up. B4 books what B1 says; it does not recompute it and
  it does not disagree with it.

This note records the surface, the chart of accounts, the journal and its
invariant (with the property-test plan the queue item asks for in its own
section), the posting rule for every document type, expenses and receipts,
the bank and reconciliation, period locking, the four reports, the first
scoped role alo has ever had, the error map, the tenancy rules, and the
out-of-scope list — each central decision with the alternative it rejects.

> **Wave gate, still flagged for a human, and now four waves deep.**
> `ROADMAP.md` gates wave B2 on "B1 live with ≥1 real tenant". B1, B2, BI-1
> and B3 are all code-complete and undeployed. A design note is exactly the
> work that belongs ahead of an unmet gate; **B4.02 is the first item that
> writes a migration**, and this wave is the one where shipping late costs
> most — a ledger that opens six months after the invoices it should have
> booked needs a backfill and an opening balance, which is § "When the books
> open" below rather than an accident.

## Surface

- **Inputs:** authenticated workspace users driving `/finance/*` on
  `alo-jmap` — the chart of accounts, manual journal entries, expense claims
  and their approval, mileage, uploaded receipts, uploaded bank statements
  (CAMT.053, MT940, CSV), the confirmation of a match, the period lock, and
  the four reports. Plus **one input nobody drives**: the postings written
  automatically inside somebody else's transaction, when an invoice is
  issued, a payment recorded, a credit note raised, a bill approved or an
  expense reimbursed.
- **Outputs:** JSON resources; CSV for every report and for the journal
  itself; balanced journal entries; **proposed** matches and **draft**
  expense categorisations, which are suggestions until a human says
  otherwise.
- **Who calls it:** `web/src/finance` (B4.13) calls `alo-jmap`; the `alo-ai`
  finance module (B4.14) produces propose-then-approve envelopes that
  `alo-jmap` executes against the same store functions; `billing_invoices`,
  `billing_payments`, `billing_bills` and `fin_expenses` call the posting
  functions **in-process, inside the transaction that made the document
  real**. Nothing external calls Finance, and Finance calls nothing external
  — no bank API, no tax portal, no rate service beyond the ECB file B1.21
  already imports from disk.

`/finance` is a **new top-level route prefix**: the production Caddyfile
needs it added at the next deploy (the standing human action `/billing`,
`/crm`, `/audit`, `/insights` and `/projects` already carry), and it must
join `API_PATHS` in `web/vite.config.ts` at B4.05b or every call 404s into
the dev SPA — the lesson S1.11, BI1.04 and B3.04 have each paid for once.
The prefix doubles as the SPA path, as all five of its siblings do.

### Routes

All under the authenticated `alo-jmap` router, following the convention
`/billing/*` established: the `authenticate` extractor, typed `Problem`
errors, the store-error map in [`billing::map_store_err`], registration in
`server.rs`. Every collection is the **second** path segment, because
`audit_action::event_for` derives a record's history from the matched route
template and `tests/audit_routes.rs` fails the build for a shape it cannot
read (the rename B3.11 paid for; it is not paid twice).

| Route | Purpose |
|---|---|
| `GET/POST /finance/accounts` · `GET/PATCH/DELETE /finance/accounts/{id}` | the chart of accounts: list (with balances for a period), add a custom account, rename, deactivate. Deleting an account that carries a posting is a `409` — a chart is history, not a preference (B4.02) |
| `GET /finance/entries?from&to&account&source&kind` | the journal, newest first, with its postings (B4.03) |
| `GET /finance/entries/{id}` | one entry, its postings, and the document it came from |
| `POST /finance/entries` | a **manual** journal entry: the accountant's escape hatch. Balanced or `422`, with an optional Drive attachment |
| `POST /finance/entries/{id}/reverse` | the only correction: a mirror entry dated today or later. There is no `PATCH` and no `DELETE` on this collection, deliberately |
| `GET /finance/entries.csv?from&to` | the journal as a file for the accountant's own tooling |
| `GET/POST /finance/categories` · `PATCH/DELETE /finance/categories/{id}` | expense categories and the account each books to (B4.05a) |
| `GET/POST /finance/expenses` · `GET/PATCH/DELETE /finance/expenses/{id}` | expense claims — the caller's own through the account door (B4.05a) |
| `POST /finance/expenses/{id}/submit` · `/approve` · `/reject` · `/reimburse` | the four transitions; the last three are approver-only (B4.05b) |
| `POST /finance/receipts` | upload a receipt, get the **parsed fields back for confirmation**. Writes no expense (B4.06b) |
| `GET/POST /finance/mileage` · `DELETE /finance/mileage/{id}` | mileage claims; each becomes an expense at the tenant's per-km rate (B4.07) |
| `GET/PUT /finance/mileage/rates` | the per-km rate table, effective-dated (B4.07) |
| `POST /finance/imports/bank` | a statement file (CAMT.053, MT940, or CSV with a mapping) → a statement header and staged lines (B4.08) |
| `POST /finance/imports/bank/preview` | the CSV mapping wizard's dry run: columns, sample rows, what would import. Writes nothing, and joins `READ_ONLY_POSTS` beside `/crm/imports/leads/preview` (B4.08c) |
| `GET /finance/bank/statements` · `GET /finance/bank/lines?status=&statement=` | what was imported and where each line stands (B4.08) |
| `GET /finance/bank/lines/{id}/suggestions` | the ranked match candidates for one line — a read, never a write (B4.09) |
| `POST /finance/bank/lines/{id}/match` · `/unmatch` · `/ignore` | confirm a suggestion (which is what creates the payment and its postings), undo it, or say this line is not ours to book (B4.09c) |
| `GET/POST /finance/rules` · `DELETE /finance/rules/{id}` | the per-tenant learned matching rules, listed and editable because a rule nobody can read is a rule nobody can trust (B4.09b) |
| `GET /finance/periods` · `POST /finance/periods/lock` · `/unlock` | the fiscal periods and the soft close (B4.10) |
| `GET /finance/reports/pl?from&to` · `.csv` | profit and loss (B4.11a) |
| `GET /finance/reports/balance?on` · `.csv` | balance sheet at a date (B4.11b) |
| `GET /finance/reports/aged?on&side=receivable\|payable` · `.csv` | aged receivables and payables (B4.11c) |
| `GET /finance/reports/vat?from&to` · `.csv` | the VAT-return figures (B4.11d) |

Twelve path segments are reserved words under `/finance` — `accounts`,
`entries`, `categories`, `expenses`, `receipts`, `mileage`, `imports`,
`bank`, `rules`, `periods`, `reports`, `settings`. Ids are base64url'd
16-byte random tokens (`id.rs`), so a record can never *be* one of them, and
matchit prefers a static segment to a capture — the shape `/tasks/labels`
beside `/tasks/{id}` has had since ADR 0021.

**There is no `POST /finance/entries/{id}` and no route that posts a
document.** Posting is not a verb a client may use: an entry exists because
a document became real, and the only door to that is the document's own
route. The one exception is the manual entry above, which is a document of
its own kind and says so in `kind = 'manual'`.

### Web surface

`web/src/finance`, the module pattern Billing, CRM, Insights and Projects
share: one `FinanceModule.tsx` owning the tabs, one `api.ts` owning the
fetches, one view per screen, `.module.css` for layout, `ds` tokens for
everything else, every string through `i18n/en.ts` under a `finance*` prefix
(fr/nl at the wave review, B4.15).

- **Expenses** — my claims, and (for an approver) everybody's: submit,
  approve, reject, mark reimbursed. This is the screen most employees will
  ever see of the module, so it is the one the empty state onboards.
- **Bank** — import a statement, then the reconciliation screen: unmatched
  lines on the left, the suggestion and its evidence on the right, one
  confirm per line and an undo beside it.
- **Accounts** — the chart, editable, with each account's balance for the
  period; the journal behind it, filterable, with the CSV button.
- **Reports** — P&L, balance sheet, aged receivables/payables, VAT return;
  each a table with a CSV button and the period picker the VAT summary
  (B1.20) already uses.

Four tabs, not five: the manual journal entry is a dialog on the Accounts
tab rather than a screen, because it is the rarest action in the module and
a screen for it would rank it above the four things people do daily.

## The chart of accounts (B4.02)

```
fin_accounts          (grain: account)
  tenant_id, id       PK
  code                TEXT — what an accountant types; UNIQUE (tenant_id, code)
  name                TEXT
  type                'asset' | 'liability' | 'equity' | 'income' | 'expense'
  role                TEXT, '' for an ordinary account; UNIQUE per tenant when set
  active              BOOLEAN
  system              BOOLEAN — seeded by us; renameable, never deletable
  created_at, updated_at

fin_seeds             (grain: tenant × seed) — the insight_seeds pattern, exactly
  tenant_id, system_key PK, seeded_by, seeded_at
```

Two decisions carry this table.

**A posting rule finds its account by `role`, never by code.** `role` is a
closed set of our own words — `ar`, `ap`, `bank`, `cash`, `vat_output`,
`vat_input`, `revenue`, `expense_default`, `employee_payable`, `fx_diff`,
`rounding`, `opening_balance`, `suspense` — and exactly one account per
tenant may hold each. So "where does the receivable go" is answered by a
lookup that survives a tenant renumbering their whole chart to match their
accountant's, which they will.

*Rejected: hardcoding account codes in the posting rules.* A code is a
national convention (SKR03 and SKR04 disagree with each other before either
disagrees with the French PCG), and a rule that knows `1400` is a rule that
is wrong in every country but one, silently, in the direction of a
misfiled tax return.

**The default chart is a neutral EU-SME chart, seeded per tenant on first
read of `/finance/accounts`, recorded in `fin_seeds` so a tenant who deletes
it is not handed it again the next morning** — the BI1.06 mechanism, reused
whole, including the primary key that makes two simultaneous first visits
race-free (`ON CONFLICT DO NOTHING`, and the winner writes the accounts).

*Rejected: shipping national charts (SKR03/04, PCG, the Belgian MAR) in B4.*
Each is large, some are the property of somebody else, and every one of them
is a **compliance claim** — publishing a chart labelled "SKR04" asserts it is
SKR04, which a loop iteration is not entitled to assert. The neutral chart
plus roles gets a tenant booking correctly on day one, and adopting a
national chart is a recode their accountant does with a CSV import (named in
out-of-scope, flagged for a human).

**VAT is a dimension on the posting, not an account per rate.** One
`vat_output` and one `vat_input` account; the rate travels in
`fin_postings.vat_rate_bp`. *Rejected: an account per rate*, which is what
many charts do — a rate change (Germany's 19→16→19 in 2020, and the ViDA
changes now queued) then mints new accounts, and every report must know both
the old and the new to add up one year.

## The journal (B4.03)

```
fin_entries           (grain: entry — one document event)
  tenant_id, id       PK
  entry_date          DATE — the accounting date (the document's tax point)
  kind                'invoice' | 'credit_note' | 'payment' | 'bill' |
                      'bill_payment' | 'expense' | 'reimbursement' | 'mileage' |
                      'manual' | 'opening' | 'reversal'
  source_kind         '' | 'invoice' | 'payment' | 'bill' | 'expense' | 'bank_line'
  source_id           TEXT — the document
  source_event        TEXT — 'issue', 'void', 'settle', 'approve', 'reimburse'
    UNIQUE (tenant_id, source_kind, source_id, source_event) WHERE source_kind <> ''
  memo                TEXT
  reverses_entry_id   TEXT nullable → fin_entries
  attachment_node_id  TEXT nullable → a Drive node (manual entries)
  currency            TEXT — the document's currency
  fx_base_currency, fx_rate_micro, fx_rate_date   — the B1.21 snapshot triple
  created_by, created_at

fin_postings          (grain: posting — one line of one entry)
  tenant_id, id       PK
  entry_id            → fin_entries (composite FK, ON DELETE CASCADE)
  position            INTEGER
  account_id          → fin_accounts (composite FK, ON DELETE NO ACTION)
  amount_cents        BIGINT signed: positive = debit, negative = credit
  base_cents          BIGINT signed — the same money in the tenant's base currency
  vat_rate_bp         INTEGER nullable — which rate this tax belongs to
  customer_id         TEXT nullable   \
  supplier_key        TEXT nullable    | the dimensions a report groups by:
  project_id          TEXT nullable    | who, what engagement, whose expense
  user_id             TEXT nullable   /
  memo                TEXT
```

**As built (B4.03a), one correction to the line above:** the account link is
`ON DELETE NO ACTION` (the default) rather than the `ON DELETE RESTRICT` this
note first wrote. The rule it exists for is unchanged — deleting an account
that carries a posting fails with SQLSTATE 23503, which
`delete_fin_account` maps to "an account that carries postings cannot be
deleted" — but RESTRICT is checked *immediately*, so dropping a whole tenant
could fail depending on which of the two cascades from `tenants` Postgres runs
first. NO ACTION is checked at the end of the statement, by which time the
postings are gone too. This is 0106's lesson, and the tenancy suite deletes a
tenant with a journal to prove it.

There is no `updated_at` on either table, and that is a signal rather than an
omission: **nothing in this module ever updates a posted row.** The columns
that would need one do not exist.

### Signed amounts — the decision

**One signed `amount_cents`: positive is a debit, negative is a credit, and
the invariant is `Σ = 0`.**

*Rejected: separate `debit_cents` and `credit_cents`, each non-negative, at
most one non-zero* — the shape most textbooks draw and several ledgers store.
It costs a `CHECK` that only one is set, doubles every aggregate into
`SUM(debit) - SUM(credit)`, and turns the invariant from "adds to zero" into
"two sums are equal", which is the same statement written so that a reader
must check which column a report forgot. Signs are how the arithmetic is
actually done, and `billing_bills` already stores a credit note in **ledger
direction** (negative) for exactly this reason — 0111 says so in its header.
The debit/credit words survive where humans read them: the journal screen and
the CSV render two columns from the sign.

### Immutable, append-only, single-writer — the decision

**An entry is written whole, in one transaction, by one store function
(`fin_journal::post`), and is never updated or deleted afterwards.** There
is no API to add a posting to an existing entry, and none to change one.

This is not conservatism; it is what makes the invariant enforceable at all.
An entry that can never be edited can only be unbalanced at the instant it is
written, and exactly one function writes it — so one check, in one place,
covers every path forever. A correction is a **reversal**: a mirror entry
with `reverses_entry_id` set, dated on or after the original, which is what
an auditor expects to see and what a void or a credit note already does one
layer up.

*Rejected: a deferred `plpgsql` constraint trigger checking `SUM = 0` per
entry at commit.* It is the textbook defence-in-depth answer and it was
seriously considered. Three reasons against: CLAUDE.md's two-language rule
(a third language in our repos is a bug, and this repo currently contains no
stored procedure at all); a trigger duplicates the rule in a place our test
suite does not reach the way it reaches Rust; and it can only see rows, not
intent — it would happily pass an entry that balances and books the wrong
account. What replaces it is cheaper and catches more:

- the write path refuses an unbalanced entry with a typed `Validation`, and
  it is the only write path;
- `fin_journal::unbalanced_entries()` — `GROUP BY entry_id HAVING SUM(...)
  <> 0` — is a store function that the test suite asserts is empty after
  every property run, and that the admin health surface can call on a live
  tenant;
- the property tests in § below, which are the real defence.

### Idempotency, because a document must post exactly once

`UNIQUE (tenant_id, source_kind, source_id, source_event)` is the whole
mechanism. Issuing invoice X posts `('invoice', X, 'issue')`; a retry, a
double-click, or a re-run of the backfill hits the constraint and is a typed
`Conflict`, not a second set of postings. Voiding it posts `('invoice', X,
'void')` — a different event, so the reversal is representable without
weakening the key.

*Rejected: a `posted` boolean on the document.* Two places to be right, and
the one that is written outside the posting transaction is the one that lies
after a crash.

### Two currencies, and the balancing figure

Every posting carries both the document's currency amount and its base
equivalent, crossed through `billing_fx::convert_cents` at the snapshot rate
frozen on the entry — the same rate the document itself carries (B1.21, EU
VAT Directive art. 91: the rate is fixed at the tax point and an auditor
recomputes from the number printed on the paper).

**An entry must balance in the base currency, always, and in each document
currency present.** Two facts follow that a reader would otherwise trip on:

- **Conversion is per posting, and the residual has a home.** Rounding is
  not linear, so converting each posting at one rate can leave the base
  column off by a cent or two even though the document column is exact. The
  residual is posted to the `rounding` account, never absorbed into whichever
  posting happens to be last — that would misstate a real account to tidy an
  arithmetic artefact. A residual larger than one cent per posting is an
  internal error, not a rounding: the write refuses. This is
  `billing_fx::convert_totals`' own doctrine (cross the parts, sum the parts,
  never cross the whole) applied to postings.
- **A posting may have `amount_cents = 0` if and only if `base_cents ≠ 0`.**
  That posting is the exchange difference: a payment settling a USD invoice
  moves exactly the invoice's dollars (so the document column balances) but a
  different number of euro than the invoice was booked at (so the base column
  does not). The difference goes to `fx_diff`. Both columns zero on one
  posting is refused — a posting that moves no money in either currency is a
  typo.

## The invariant, and how it is proven (B4.03b)

The queue item asks for the debits==credits invariant "stated as a property
test plan". Here it is. `alo-store`'s tests run against the real database, so
these are integration properties, not pure-function ones; the generator lives
in the test module and is seeded deterministically so a failure is
replayable.

**The generator** produces a random *business month* for one tenant: 1–20
customers and suppliers, 1–50 invoices with 1–20 lines each (random
quantities in milli-units, random unit prices in cents, VAT rates drawn from
{0, 500, 700, 900, 1900, 2100, 2500} bp), a random subset issued, a random
subset of those credited, 0–3 payments per issued invoice (partial, exact, and
deliberately over), 0–30 approved bills, 0–30 expense claims across random
categories and payment methods, 0–10 mileage claims, in 1–3 currencies with
random-but-valid FX snapshots. Everything is generated through the **real**
store functions, never by inserting rows: a property that holds only for
hand-built fixtures proves nothing about the code that will run.

| # | Property | Why it is the one that would catch the bug |
|---|---|---|
| **P1** | Every entry balances: `Σ amount_cents = 0` per currency present, and `Σ base_cents = 0`. | The invariant itself. Asserted per entry *and* re-derived from the database with `unbalanced_entries()`, so an in-memory check that lies about what was written is caught. |
| **P2** | Every posting is non-trivial: `amount_cents ≠ 0` or `base_cents ≠ 0`, never both zero. | The FX-difference exception cannot become a hole. |
| **P3** | An issued invoice's AR posting equals `billing_totals::totals(lines).gross_cents` **exactly**; its VAT postings per rate equal that struct's per-rate subtotals; its revenue postings equal net. | The ledger books what B1 computed. If B4 ever recomputes totals, this fails — which is the point. |
| **P4** | For every document and its full credit note, the postings of both sum to zero **per account and per dimension**. | B1.09's "ledger of original + credit sums to zero", now true of the ledger and not only of the documents. |
| **P5** | After any sequence of payments totalling the gross, the AR balance attributable to that invoice is zero; after a partial, it equals the outstanding `billing_payments::Settlement` reports. | The two sub-systems agree about what is still owed. |
| **P6** | At any date, the `ar` account's balance equals the sum of outstanding issued invoices at that date, and `ap`'s equals the sum of unpaid approved bills. | The reconciliation between the ledger and the documents, asserted rather than assumed — and the reason B4 does **not** keep an open-item sub-ledger (§ Reports). |
| **P7** | Posting the same document event twice yields exactly one entry and a typed `Conflict`; the tenant's total posting count is unchanged. | Idempotency, including under the backfill. |
| **P8** | No sequence of API calls changes or removes a posted row: after the run, every posting's `(entry_id, account_id, amount_cents, base_cents)` tuple written earlier is still present byte-for-byte. | Append-only, proven behaviourally rather than by reading the source. |
| **P9** | Tenant A's generated month leaves tenant B's every balance, report and journal read **identical to before** — and A's ids are `NotFound` through B's handle. | Law 1, in the form that catches an aggregate missing a `tenant_id` rather than a single read. |
| **P10** | Every figure on the P&L, the balance sheet and the VAT report equals the same figure derived independently from the generated documents; and the balance sheet balances (`assets = liabilities + equity + P&L`). | The reports are the journal added up, and the journal is the documents. All three agree or the build is red. |

Two further tests that are not properties but belong beside them: a
**hand-written golden** entry per document type (B4.04a–c) so a reader can
see the expected debits and credits written out by a human, and a **no-float
grep test** in the same shape the billing suite already carries — money is
`i64` cents, rates are basis points, FX is micro-units, and `f64` appears
nowhere on the path.

### As built (B4.03b)

`tests/fin_journal_properties.rs` holds **P1, P2, P4, P7, P8 and P9** plus a
sixth test for the period window. P3, P5, P6 and P10 are each an assertion
about a *rule* or a *report*, and neither exists yet: they land with B4.04a–c
and B4.11a–d respectively rather than being weakened into something that
passes today. What stands in for them in the meantime is not a stub — the
generator keeps its own running tally as it posts, and `fin_trial_balance`,
`fin_dimension_balances` and `fin_account_ledger` are each checked against it
account by account, customer by customer and rate by rate. That is P10's
shape one layer below the reports.

The generator is the note's, with one honest narrowing: it produces the
*entries* B4.04's rules will produce (issue, settle, approve, credit) rather
than the *documents*, because there is no rule yet to turn a document into an
entry. Everything still goes through `post_fin_entry`, never an insert. Four
seeds run per invocation, and the month each produces must contain a
foreign-currency document, a rounding residual and an exchange difference —
asserted, because a generator that quietly stops producing the hard case
leaves a suite that is green and tests nothing.

Both queries were **mutation-checked** rather than merely run: summing the
document column instead of the base column fails three tests, and dropping
the `tenant_id` predicate from the ledger read fails P9.

### The posting-query API (B4.03b), which the reports are folds over

`fin_ledger.rs` is the read side, and it is deliberately three functions
rather than four reports' worth of queries:

| Function | Answers | Feeds |
|---|---|---|
| `fin_trial_balance(from, to)` | what every account moved in a window, with the two totals that must be equal | P&L (income + expense for a period), balance sheet (no lower bound, at a date) |
| `fin_account_ledger(account, from, to, limit)` | one account line by line, with the opening balance and a running column | every drill-down; `flag_anomalies` (B4.14b) |
| `fin_dimension_balances(scope, dimension, from, to)` | what each value of one dimension moved, over the accounts a scope names | receivables by customer, payables by supplier, cost by engagement, VAT by rate |

Three decisions inside them a later wave should not relitigate. **Every
aggregate is in the accounting currency** (`base_cents`): a total that adds
dollars to euro means nothing, and one that silently reports the majority
currency means something false. **A scope names a `role`, not a code** — so
receivables-by-customer stays right after an accountant recodes the chart.
And **the dimension is a closed Rust enum mapping to a column name written
here**, which is what keeps the one interpolated fragment in the module safe
by construction; a unit test asserts every variant is a plain posting column
and that a hostile account id travels as a bind parameter.

Two caps, both visible rather than silent: `LEDGER_PAGE_MAX` on a drill-down
and `LEDGER_GROUPS_MAX` on a grouped read, each with a `truncated` flag,
because a partial sum presented as a period's total is the one defect a
financial report must not have.

## Posting rules, per document type (B4.04)

Each rule is a pure function from a document to a `NewEntry` — no database
access, no clock, no tenant lookup beyond the account roles handed to it — so
every one of them is unit-testable against a hand-written golden before it is
ever wired into a transaction. `fin_rules.rs` owns them; `fin_journal.rs`
owns writing what they return.

| Event | Debit | Credit |
|---|---|---|
| **Invoice issued** (B4.04a) | `ar` gross, dimension `customer_id` | `revenue` net (per line, dimension `project_id` when the line came from B3); `vat_output` per rate, dimension `vat_rate_bp` |
| **Credit note issued** (B4.04c) | `revenue`, `vat_output` — the exact mirror | `ar` |
| **Payment recorded** (B4.04b) | `bank` (or `cash` — by the method map) | `ar`, dimension `customer_id`; plus `fx_diff` for the base-column difference |
| **Bill approved** | `expense_default` or the category's account, net; `vat_input` per rate | `ap`, dimension `supplier_key` |
| **Bill paid** (the SEPA run's line, reconciled) | `ap` | `bank` |
| **Expense approved**, paid by the employee | the category's account, net; `vat_input` | `employee_payable`, dimension `user_id` |
| **Expense approved**, paid by company card | the category's account, net; `vat_input` | `bank` |
| **Expense reimbursed** | `employee_payable`, dimension `user_id` | `bank` |
| **Mileage approved** | the mileage category's account (no VAT — a per-km allowance carries none) | `employee_payable` |
| **Manual entry** | whatever the accountant typed | whatever the accountant typed — balanced or refused |
| **Opening balances** | per account, as stated | `opening_balance` as the counterweight |

Four rules about *when*, which matter more than the table:

- **A draft posts nothing.** Draft invoices, unsubmitted expenses,
  unapproved bills and staged bank lines are not events; they are intentions.
  The ledger learns about a document the moment it becomes irrevocable —
  which for an invoice is `issue` (B1.08's gapless number), for a bill is
  `approve`, for an expense is `approve`, and for money is the day the bank
  says it moved.
- **The posting happens inside the document's own transaction.** Issuing an
  invoice and booking it either both happen or neither does. A "post later"
  queue would create the state this module exists to make impossible: a legal
  document that is not in the books.
- **A posting failure fails the document.** If the chart is missing a role
  the rule needs, the *issue* is refused, naming the role and pointing at the
  Accounts screen. *Rejected: issuing anyway and posting to `suspense`* — a
  suspense account is for money whose owner is unknown, not for a
  configuration mistake, and a silent suspense posting is discovered at the
  year end by somebody who cannot remember the invoice.
- **`entry_date` is the document's date, not today.** The invoice's issue
  date, the payment's `paid_on`, the expense's date — a ledger keyed on when
  a clerk typed is a ledger no period report can trust. This is what makes
  the period lock (B4.10) load-bearing rather than decorative.

### As built (B4.04a), the invoice rule

`fin_rules::invoice_issue_entry` is the table's first row, and
`fin_booking.rs` is the thin layer that reads the document, resolves `ar`,
`revenue` and `vat_output` **by role**, applies the rule and posts what it
returns (`AccountStore::post_invoice_issue`; `fin_invoice_entry` answers "is
this document booked?" without catching a conflict). Four things it does that
this note did not say in advance, each a decision rather than a shortcut:

- **Revenue is one posting per VAT rate, not one per line.** The table above
  says "per line, dimension `project_id` when the line came from B3" — and a
  billing line carries no project today (`billing_line::Line` is description,
  quantity, price, rate; B3's handoff writes the hours into the description).
  Per-line postings would turn a 400-line invoice into 400 identical credits
  carrying nothing the rate grouping does not already carry. When a line gains
  a project link, the rule splits the per-rate credit by project and nothing
  else about it changes.
- **The rate travels on the revenue posting too**, not only on the tax
  posting. A VAT return needs the taxable base per rate as well as the tax per
  rate, and taking both from the journal is what makes the return and the
  books provably one statement rather than two that agree today.
- **The receivable's base amount is the sum of the crossed parts**, never the
  crossed gross — `billing_fx::convert_totals`' doctrine with the parts being
  the revenue and tax postings. So the invoice rule can never leave a
  rounding residual, and the receivable the books carry
  is to the cent the figure `billing_fx::restated_into` reports for the same
  document — which is what P6 will need. Both suites pin this with a rate
  where the two ways of doing it **disagree** (1 EUR = 1.0880 USD on the
  golden document: €1 201.28 the parts, €1 201.29 the whole), because an
  example where they agree proves nothing about which one the code does.
- **A `paid` invoice is bookable and a `void` one is not.** Paid was issued
  first, and a backfill meets documents that have since been settled; void is
  booked by its issue entry and corrected by its `void` reversal (B4.04c's
  neighbour), so booking it as an issue alone would misstate the ledger.
  Draft and credit note are refused as `Conflict`, naming the rule that owns
  them.

**P3 is now asserted**, in both shapes the note asked for: a hand-written
golden entry in `src/fin_rules.rs` (the debits and credits written out as an
accountant writes them) and, on the wire in `tests/fin_invoice_posting.rs`,
the receivable equal to `billing_totals::totals(lines).gross_cents` with the
per-rate postings equal to that struct's own rows — checked against a total
this suite computes independently, and again one layer up in
`fin_trial_balance`.

**Not yet wired into `issue_billing_invoice`.** The rule above ("the posting
happens inside the document's own transaction") stands, and this is the
function that transaction will call — but making it fire today would make
issuing depend on a chart the tenant has never visited and on a books-opening
date that does not exist until B4.10. So the wiring lands with the periods and
the backfill, and until then the caller is explicit. This is a cut with a
date, not a permanent seam.

### As built (B4.04b), the settlement rule

`fin_rules::payment_settle_entry` is the table's third row, and
`AccountStore::post_payment_settle` is its booking layer
(`fin_payment_entry` answers "is this payment in the books?").

- **The two money legs are crossed at two different rates, and that is the
  whole point.** The bank leg is what the accounting currency actually
  received, so it crosses at the rate published for the day the money arrived;
  the receivable leg has to remove what the *invoice* put there, so it crosses
  at the rate frozen on the document (art. 91 again: the tax point's rate is
  the document's rate forever). The difference is the gain or loss made by
  being paid later, and it is posted to `fx_diff` on its own line with
  `amount_cents = 0` — the one posting shape the journal's "moves no money"
  rule deliberately allows.
- **The receivable relieved is cumulative, not per payment.** The rule is
  handed the total paid *before* this payment; the relief is what the whole
  prefix relieves minus what the shorter one did. That makes a settled
  document's receivable go to **exactly** zero in both columns, because the
  last payment carries the cent by which the crossed gross differs from the
  sum of crossed parts the issue entry booked. Relieving each payment at the
  plain crossed amount instead leaves a one-cent phantom receivable that no
  aged-debtors report can explain and no payment can ever clear; both suites
  pin this with a mutation-tested golden ($1 307.00 at 1.0880 settled at
  1.1000 and 1.0500). *So `rounding` is still unused:* every cent a settlement
  leaves over is an exchange difference, which has a better-named home.
- **The `fx_diff` role is required only when it can be reached** — that is,
  when the document is not in the accounting currency
  (`settlement_needs_exchange_account`). A chart missing it must not refuse an
  ordinary euro payment over a posting that rule provably never writes.
- **A payment refuses to book before its invoice does** (`Conflict`), because
  relieving a receivable nobody booked leaves the customer's ledger negative.
  Ordering within a document is `billing_payments`' own order read back to
  front: the reliefs telescope under *any* stable order, so booking payments
  out of sequence is safe.
- **The method map is a closed default, matched on whole words.**
  `payment_settlement_role` reads the words that mean physical cash (en/fr/nl
  plus German) as `cash` and everything else as `bank` — never a substring,
  because "cashless" is the bank. The per-tenant table this note promises
  replaces the constant behind the same signature when the Accounts screen
  grows a place to edit it.

**Not yet wired into `record_billing_payment`**, for the same reason and with
the same date as the invoice rule. One consequence to carry into that work:
`delete_billing_payment` is B1's correction path, so once payments book
automatically, deleting a booked one must post a **reversal** (which
`fin_journal` already supports) rather than silently leaving its entry
behind.

### When the books open

A tenant that has been invoicing since B1 has documents older than its
ledger. Two mechanisms, both explicit, neither automatic:

- **`POST /finance/periods/open`** — an admin states the day the books
  begin. Documents dated before it are never posted; the balances they left
  behind arrive as an **opening-balances manual entry** the accountant
  writes (kind `opening`, counterweighted by the `opening_balance` account),
  which is exactly how a business migrating from another ledger opens ours.
- **A backfill**, run once per tenant and recorded in `fin_seeds`, posting
  every already-real document dated on or after that day, in date order,
  through the same rules. P7 is what makes running it twice harmless.

*Rejected: posting retroactively for every document that ever existed.* A
tenant's first two years may be in another system entirely; booking them here
would produce a second, disagreeing set of books for a period an accountant
has already closed and filed.

## Expenses, receipts and mileage (B4.05–B4.07)

```
fin_categories   tenant_id, id PK, name, account_id → fin_accounts,
                 default_vat_rate_bp nullable, active
fin_expenses     tenant_id, id PK, user_id (whose claim), spent_on DATE,
                 category_id, merchant TEXT, description TEXT,
                 gross_cents, vat_cents, vat_rate_bp, currency,
                 method 'personal' | 'card' | 'cash',
                 project_id nullable (the B3 bridge, and the rebill hook),
                 receipt_node_id nullable → a Drive node,
                 status 'draft'|'submitted'|'approved'|'rejected'|'reimbursed',
                 submitted_at, decided_by, decided_at, decision_note,
                 reimbursed_on, created_at, updated_at
fin_mileage      tenant_id, id PK, user_id, travelled_on, km_milli,
                 rate_cents_per_km (snapshotted), from_place, to_place, reason,
                 expense_id → fin_expenses
fin_mileage_rates tenant_id, id PK, effective_from DATE, cents_per_km, note
```

**An expense claim is personal data about an employee, and it uses B3's two
doors unchanged.** A person creates, edits, submits and withdraws their own
claims through `AccountStore` (every statement carries `user_id = self.user`,
so a colleague's claim is unrepresentable rather than merely forbidden); an
approver reads and decides through `TenantStore` behind a role gate. B3's
note wrote the rule for hours; a receipt is worse — it names a restaurant, a
pharmacy, a city on a date — and the same door answers it.

**Every field a machine extracted is confirmed by a human before it books.**
`POST /finance/receipts` returns parsed fields and **writes no expense**; the
client shows them in the create form for editing; the create call is an
ordinary create. *Rejected: creating a draft expense from the upload* — a
draft in a list is a thing somebody approves without reading, and the whole
value of the confirmation step is that the numbers are looked at once by
somebody who was there.

**The extractor is a trait with a deterministic implementation first**
(B4.06a): text layer or filename and a set of well-tested European receipt
patterns — dates in six spellings, `Summe`/`Total`/`Totaal`/`Montant`, VAT
lines with a rate beside them, IBANs, amounts with a comma decimal. It
returns *fields with confidences and the span each came from*, so the UI can
show why. An AI backend is a second implementation of the same trait, wired
by a human (ADR 0029, EU-only inference); **the loop never calls a model**,
and the fixture suite is what proves the seam.

**VAT on an expense is stated, never derived.** A receipt showing €119 with
19% is entered as gross 119 / VAT 19; a receipt showing only a total is
entered with VAT 0 and books nothing to `vat_input`. *Rejected: computing the
VAT from the gross and a category default* — reclaiming input VAT that the
receipt does not evidence is a false statement on a return, and the
difference between "the receipt does not show it" and "the receipt shows
zero" is exactly the difference a tax inspector asks about.

**Mileage is a claim at a rate table, not an expense with a made-up amount.**
`km_milli × cents_per_km` in integer arithmetic, rate snapshotted at the rate
effective on the travel date, and the resulting expense is ordinary from
there. Whether a given per-km rate is tax-free in a given member state is a
**human's statement**: the table ships empty with a note, not pre-filled with
Germany's €0.30.

## The bank (B4.08) and reconciliation (B4.09)

```
bank_statements  tenant_id, id PK, account_iban, currency, source 'camt'|'mt940'|'csv',
                 file_sha256, opening_balance_cents, closing_balance_cents,
                 from_date, to_date, imported_by, imported_at
                 UNIQUE (tenant_id, file_sha256)
bank_lines       tenant_id, id PK, statement_id, line_no,
                 booked_on DATE, value_on DATE, amount_cents signed, currency,
                 counterparty_name, counterparty_iban, remittance TEXT,
                 bank_ref TEXT, line_hash TEXT,
                 status 'unmatched'|'matched'|'ignored',
                 UNIQUE (tenant_id, line_hash)
bank_matches     tenant_id, id PK, line_id, target_kind 'invoice'|'bill'|'expense'|'entry',
                 target_id, amount_cents, confirmed_by, confirmed_at, rule_id nullable
fin_match_rules  tenant_id, id PK, match_on 'counterparty'|'remittance'|'iban',
                 pattern TEXT, target_kind, account_id nullable, customer_id nullable,
                 supplier_key nullable, hits INTEGER, created_by, created_at
```

**Three parsers, one contract.** CAMT.053 is XML and goes through
`billing_xml_tree` — the reader B1.24 hardened against DTDs, entity
expansion, unbounded depth and prefixes, which is precisely the tool for a
file a bank generated with software we have never seen. MT940 is a line
format (`:61:` statement lines, `:86:` remittance, `:60F:`/`:62F:` balances)
and gets its own small parser. CSV goes through `csv_read` — BOM and
encoding detection, delimiter sniffing, the CP1252 fallback that is half the
files in Europe — plus a mapping the user confirms in a preview. All three
produce the same `ParsedStatement`, and each has golden files from public
samples. A malformed file is a typed `422` naming the line, never a partial
import.

**A file imports once and a line imports once.** The statement's SHA-256 and
the line's hash (bank reference, date, amount, remittance, normalised) are
unique per tenant. Re-uploading last month's file is a `409` naming the
import; a statement that legitimately overlaps another loses only its
duplicate lines, and the import report says how many and why. *Rejected:
trusting the bank's own reference alone* — some banks reuse it, some omit it.

**Matching is three stages, and only the first is arithmetic.**

1. **Exact** (B4.09a): amount, sign and currency equal, and the remittance
   contains our own invoice number (`INV-YYYY-NNNNN`, which B1.08 makes
   unambiguous) or a bill's number, within a date window. This is the only
   stage that can be right by construction, and it is still a *suggestion*.
2. **Heuristic** (B4.09b): amount within a window of days, counterparty name
   similar to a customer or supplier, or a learned rule that fired before.
   Ranked, with the evidence shown — "amount and IBAN match, 3 days late".
3. **Manual** (B4.09c): the human picks, or marks the line `ignored` with a
   reason.

**Nothing is ever auto-confirmed** — ADR 0023's rule, and here it is also a
money rule: a wrong automatic match marks an invoice paid that is not, and
the customer stops being chased. Confirming a match is what creates the
`billing_payments` row (which posts, through B4.04b's rule), links the line,
and — if the user asks — writes the rule that would have matched it next time.
Rules are listed, editable and deletable, and each shows its hit count,
because a learned rule nobody can read is a rule nobody can trust.

**Unmatching is real.** `POST .../unmatch` deletes the payment it created and
**reverses** its entry (a reversal, never a deletion), returning the line to
`unmatched`. Refused once the period is locked, which is the next section.

## Fiscal periods and the soft close (B4.10)

```
fin_periods   tenant_id, id PK, from_date, to_date, status 'open'|'closed',
              closed_by, closed_at, note
              — plus the derived tenant-level lock date: max(to_date) of closed
```

**Posting with `entry_date` on or before the lock date is a typed error, and
so is the document action that would have caused it.** Issuing an invoice
dated in a closed quarter is refused at `issue`, naming the lock date, not
half-completed with a floating entry. A close is **soft**: an admin may
reopen a period, the reopen is audited with the reason, and every entry
posted afterwards is visible as such (its `created_at` is after the close it
follows). *Rejected: a hard close* — a small business finds a missing receipt
in week three of every quarter, and a lock nobody can lift is a lock people
work around by backdating the next period, which is worse.

*Rejected: deriving the lock from a single `lock_before` date in settings.*
Named periods are what the reports offer as a period picker and what an
accountant asks for by name ("is Q2 closed?"); a bare date answers the
posting question and none of the others.

## The four reports (B4.11)

All four read the **journal**, and nothing else. That is the point of having
one: a P&L that read invoices, a balance sheet that read bank lines and a VAT
return that read documents would be three systems that agree until the first
manual entry.

- **P&L** — income and expense accounts, grouped by type then code, for a
  period, with the comparative period beside it.
- **Balance sheet** — asset, liability and equity balances at a date, plus
  the period's result; it must balance, and P10 says so.
- **Aged receivables/payables** — the one report that reads **documents**
  (`billing_invoices` + `billing_payments`, `billing_bills`), bucketed
  current / 1–30 / 31–60 / 61–90 / 90+ from the due date. It reads documents
  because ageing is a property of a document, not of an account balance —
  and the invariant that keeps the two honest is **P6**, which asserts the
  ledger's `ar` and `ap` balances equal these totals. *Rejected: an
  open-item sub-ledger inside the journal* — it duplicates B1's payment state
  machine, and two implementations of "what is still owed" drift.
- **VAT return figures** — output VAT per rate, input VAT per rate,
  the net payable, from postings carrying `vat_rate_bp`. It reconciles
  against `billing_vat_report` (B1.20), which reads documents; the two are
  compared in a test on the seeded year, because "a chart and a tax return
  cannot disagree" (BI1.01) applies hardest where the tax return is literal.

CSV follows `billing_reports` exactly: ISO dates, `.` decimals, untranslated
column headers (a file read by a spreadsheet and an accountant's tooling must
not move with the reader's locale), amounts in units with two decimals, and
no personal data beyond the counterparty name a document already prints.

**These are figures for a return, not a return.** ADR 0035's non-goal is
unchanged: alo produces correct, exportable numbers; filing goes through the
national portal or a partner. The screen says so.

## The accountant role (B4.12) — alo's first scoped role

Until now alo has had two access facts: `users.is_admin` (a tenant admin) and
Spaces membership with a `SpaceRole` (ADR 0026/0028, which governs Drive and
what attaches to a Space). B4.12 adds the third, and it is deliberately the
narrowest thing that solves the actual problem: **an external accountant
needs the books and must not have the mail.**

```
tenant_user_roles   tenant_id, user_id, role PK   — role ∈ {'accountant'}
```

- Finance routes accept **admin or accountant**. Read of everything in
  `/finance/*`; write of manual entries, matches, expense decisions and the
  period lock.
- Billing and CRM are **read-only** for an accountant (they must see the
  document behind a posting); Projects, Tasks, Drive, Insights and Mail are
  unchanged — which is to say an accountant, being a user with no shares and
  no Space memberships, already sees only their own empty mailbox and no
  files. The tests prove that literally: an accountant handle gets `403` on
  every admin route, `404` on another user's expense, and nothing but their
  own on every account-door read.

*Rejected: modelling finance as a Space.* A Space is a container with
members, and the ledger is not in a container — it is the tenant. Making it
one would mean "who can see the books" and "who can see this folder" are the
same question answered by the same table, and the first tenant who removes an
accountant from a Space to tidy a sidebar would silently revoke their access
to the year-end.

**This contradicts a sentence written three times in `ROADMAP.md`, and the
contradiction is deliberate rather than overlooked.** B2.11, B3.8 and BI-1.6
each defer their access question to B4.12 with the words "designed **on
Spaces** rather than invented twice". That phrasing was written before
anybody had looked at what the first scoped role would actually be; the role
that turned up is tenant-wide and cross-module, which is the one shape a
Space cannot express. The queue item itself hedges — "via Spaces/roles". So
B4.12 delivers *roles*, and the three deferred items above are answered by
the same table when their turn comes: a per-module role is a row, and a
per-record share (which board, which engagement) is still Spaces' job and
still unbuilt. **Flagged in `docs/autonomy/STATE.md` for a human**, because
correcting three ROADMAP lines is not a loop decision.

*Rejected: a general RBAC engine (roles × permissions × resources).* We have
one role to ship. A permission matrix built for one caller is a matrix that
encodes that caller's accidents; when the second role arrives (B6's HR role
is the likely one), the table above widens by a row and the gates by a word.

**Flagged for a human:** an external accountant is still a *user*, and a user
today gets a mailbox. A no-mailbox account type is an identity change, not a
finance one, and it is named in the open questions below rather than invented
here.

## The finance agent (B4.14)

Three tools in the ADR 0034 allowlist, executed by `alo-jmap` against the
same store functions the routes use — never a parallel path — and verified
**structurally** (routes exist, guards hold, the execute path writes the
right rows against the local database). No live model call in the loop, ever.

| Tool | Kind | What it may do |
|---|---|---|
| `categorise_transactions` | **draft** | propose a category (hence an account) for unmatched bank lines or uncategorised expenses. Writes proposals; a human approves each |
| `vat_summary` | **answer** | read `/finance/reports/vat` for a period and answer with the figures as sources. No writes |
| `flag_anomalies` | **answer** | read the journal for a period and name what looks unusual — a duplicate amount to the same counterparty, an expense far outside its category's range, a month with no rent — **with the entries as sources**. No writes, no scores, no "risk" |

Two rules the third tool needs. It **names entries, never people**: an
anomaly is a fact about a document, and an agent that summarises an
employee's spending pattern is a profiling feature nobody asked for. And it
**explains every flag with the rows that caused it**, because an unexplained
flag is an accusation.

## Errors

One map, `billing::map_store_err`, used and not copied — the call CRM and
Projects both made, for the same reason: it is a store-error map, not a
billing rule.

| Condition | Store | Wire |
|---|---|---|
| no or bad token | — | `401` (the `authenticate` extractor) |
| an approver-only or accountant-only action without the role | — | `403` |
| account, entry, expense, statement, line or rule not this tenant's | `NotFound` | `404` — existence is never disclosed |
| another user's expense claim through the personal door | `NotFound` | `404` — not `403`, which would confirm it exists |
| a manual entry that does not balance | `Validation` | `422`, naming the difference in both currencies |
| a posting with both amounts zero, or an unknown account, or an inactive one | `Validation` | `422` |
| a duplicate account code, or deleting an account that carries postings | `Conflict` | `409` |
| the chart is missing a role a posting rule needs | `Validation` | `422` naming the role — and the **document action is refused**, not completed |
| posting or acting into a closed period | `Conflict` | `409` naming the period and its close date |
| a document event posted twice | `Conflict` | `409` |
| editing or deleting a posted entry | — | no such route exists; the reversal is the answer |
| a malformed CAMT/MT940/CSV, or a CSV mapping that does not cover the required columns | `Validation` | `422` naming the line and the field, never a partial import |
| re-importing a file, or a line already imported | `Conflict` | `409` with the earlier import named |
| confirming a match on a line that is already matched | `Conflict` | `409` |
| an expense transition that is not allowed from the current status | `Conflict` | `409` |
| a receipt over the size cap, or a media type we do not read | `Validation` | `422` |
| database error | `Db` | `500`, opaque — the wire never sees a raw error |

Validation messages are authored in the store and name the rule and the
field; they remain the one place a message crosses in English today — the
standing cross-cutting item B1.27, B2.14 and B3.11 each left for a human, and
this wave adds no new kind of it.

## Tenancy

Every statement carries `tenant_id` from the handle, never from request input
— the invariant `for_tenant`/`for_account` make structural rather than
remembered. Three isolation tests are mandatory, and the third is this wave's
own:

- **Wrong tenant** (law 1, every wave): tenant A's handle cannot read, post,
  match, approve, lock or report on tenant B's account, entry, expense,
  statement or line. Clean denial, not data and not a `500`.
- **Wrong user** (B3's addition, inherited): user B's `AccountStore` cannot
  read, edit, submit or withdraw user A's expense claim — a `404` inside the
  same tenant.
- **Wrong aggregate** (new here): tenant A posting a whole generated month
  leaves **every one of tenant B's balances and reports byte-identical** —
  P9. A single-row read test cannot catch a `SUM` that forgot its
  `tenant_id`; a report comparison can, and reports are what this module is.

Three data-handling rules the module adds:

- **A receipt is a file with somebody's life in it.** Receipt images live in
  Drive under the claimant's own node, referenced by id; they are never
  copied into a finance table, never logged, and an approver reads them
  through the same door that shows them the claim.
- **Nothing a human typed reaches a log.** Remittance text names people;
  expense descriptions name occasions; a counterparty name is a party to a
  transaction. `tracing` spans carry ids, counts and amounts — the rule mail
  bodies have had since Phase 1, applied to the bank.
- **`finance` joins `audit_action::AUDITED_MODULES`** beside `billing`,
  `crm` and `projects` at B4.05b, after which `tests/audit_routes.rs`
  requires every mutating `/finance/*` route to be audited by reading the
  router's own source. Sub-resource events file against their parent record
  (an approval against the expense, a match against the bank line), the rule
  B2.13 established. "Who closed the period, and when" is a question an
  auditor will ask.

## Files this wave will add

Store (`platform/alo-store/src`), one file one reason:

```
fin_accounts.rs      the chart, the roles, the default seed
fin_journal.rs       post(), the balance check, reads of the journal
fin_rules.rs         document → NewEntry, pure, one function per document type
fin_booking.rs       read the document, resolve the roles, post what the rule
                     returns (added at B4.04a: the rules stay pure and the
                     journal stays ignorant of invoices)
fin_categories.rs    expense categories and their accounts
fin_expenses.rs      claims: the account door, the approval transitions
fin_mileage.rs       the rate table and the claim it becomes
fin_receipt.rs       the extractor trait + the deterministic implementation
bank_camt.rs         CAMT.053 → ParsedStatement (over billing_xml_tree)
bank_mt940.rs        MT940 → ParsedStatement
bank_csv.rs          CSV + mapping → ParsedStatement (over csv_read)
bank_import.rs       staging, dedupe, the import report
fin_match.rs         suggestions (exact, then heuristic), confirm, unmatch
fin_rules_learn.rs   the per-tenant learned rules
fin_periods.rs       periods and the soft close
fin_reports.rs       P&L, balance sheet, VAT figures
fin_aged.rs          aged receivables/payables (the one report over documents)
migrations/0129…     fin_accounts + fin_seeds; fin_entries + fin_postings;
                     fin_categories + fin_expenses; fin_mileage(+rates);
                     bank_statements + bank_lines; bank_matches +
                     fin_match_rules; fin_periods; tenant_user_roles
```

Migration numbers are taken in order at the moment each is written — the
sites track pushes to the same branch and shares the sequence (0127 went to
sites between two of B3's), so "0129 onward" is the starting point, not a
reservation.

Routes (`products/mail/alo-jmap/src`): `finance.rs` (the module's edge
concerns and the error map reuse), `finance_accounts.rs`, `finance_entries.rs`,
`finance_expenses.rs`, `finance_receipts.rs`, `finance_bank.rs`,
`finance_match.rs`, `finance_periods.rs`, `finance_reports.rs`, plus
`agent_finance.rs` (B4.14) and the additive lines in `server.rs`, `lib.rs`
and `audit_action.rs`.

Web (`web/src/finance`): `FinanceModule.tsx`, `api.ts`, `types.ts`,
`format.ts`, `ExpensesView.tsx`, `ExpenseDialog.tsx`, `ReceiptDialog.tsx`,
`BankView.tsx`, `ImportDialog.tsx`, `MatchPanel.tsx`, `AccountsView.tsx`,
`JournalView.tsx`, `EntryDialog.tsx`, `ReportsView.tsx`, `index.ts`; the
`finance*` block in `i18n/en.ts` (fr/nl at B4.15); the module entry in
`product/workplace.tsx`; `/finance` in `vite.config.ts`.

## Out of scope for B4 (cuts are decisions)

- **Payroll calculation** — ADR 0035's explicit non-goal. B6.10 exports the
  columns a payroll provider needs; alo never computes a social-security
  contribution.
- **Tax filing** — figures, not submissions. No ELSTER, no
  impots.gouv, no Intervat.
- **National charts of accounts as shipped artefacts** (SKR03/04, PCG, MAR)
  and **DATEV export** — the latter is `[B+]` in features.md and is a
  handshake with a specific German product, not a general capability.
- **Live PSD2 bank feeds** — ADR 0035 and ADR 0009: PSD2 goes through a
  licensed aggregator, integrated by a human with a contract. What we build
  is file import, which needs no licence and works with every bank.
- **Reverse-charge and intra-community purchase VAT.**
  `billing_einvoice_import` already **refuses** a bill whose lines carry
  category `AE`, `K`, `G` or `E`, because our line holds a rate and not a
  category, and understating a return is worse than refusing a file. So a
  reverse-charge purchase cannot be imported today and therefore cannot be
  booked — the limitation is inherited, named here, and lifting it is a
  billing-side change (a category on the line) before it is a ledger one.
  **Flagged for a human**: a business buying software from another member
  state hits this in its first month.
- **Partial VAT deductibility** (the entertainment and vehicle rules several
  member states apply) — a per-category percentage is easy; knowing which
  percentage is legal is a compliance statement.
- **Depreciation, fixed-asset registers, accruals, prepayments and
  provisions** — real accounting, and each one a wave's worth of policy. The
  manual journal entry is the escape hatch until then, which is exactly what
  it is for.
- **Multi-entity consolidation, inter-company postings, cost centres beyond
  the `project_id` dimension.**
- **Automatic FX revaluation of open balances at period end** — the
  settlement difference is posted (B4.04b); revaluing what is still open is a
  policy choice per tenant.
- **Cash-basis accounting** — B4 books on the accrual basis (an invoice
  posts at issue). Several member states permit a cash basis for small
  businesses, and offering it is a per-tenant setting that changes every
  posting rule's timing: a decision, not a variation.
- **Budgets and forecasts** — that is Insights' shape (BI-2, which the
  ADR 0037 note already queues after B4).

## Open questions flagged for a human

- **Can an accountant be a user without a mailbox?** The role lands in
  B4.12; the identity model gives every user a mailbox. It is an identity
  change, and until it is made, "no mail" means "an empty mailbox and no
  shares" — which is honest but is not the same sentence features.md uses.
- **Which country's books is a tenant keeping?** The neutral chart works
  everywhere and is optimal nowhere. Whether alo ships national charts (and
  therefore makes a per-country claim) is a product and legal decision.
- **What is the retention period for the journal?** Member states require
  between 6 and 10 years for accounting records, and alo's tenant-delete
  cascade is immediate by design. A tenant leaving with unfiled books is a
  contract question before it is a schema one.
- **Should approving an expense notify the claimant?** The same question
  B3.01 left open for a rejected week, and the same answer for now: the
  status is on their screen and the module drafts nothing by itself.
- **Compliance, flagged and not guessed:** the German GoBD and the French
  and Italian equivalents require accounting records to be unalterable,
  traceable and completely retained. This design's append-only journal,
  reversal-only correction, per-event idempotency key and audit trail are
  built to that shape *deliberately* — but whether alo **claims** GoBD
  conformity (which involves a documented procedure, not only a schema) is a
  legal statement for a human, exactly as the working-time-record claim was
  in B3.
