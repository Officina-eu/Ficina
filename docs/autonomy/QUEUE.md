# QUEUE.md — Business track work queue (ADR 0035, all waves B1–B6)

Ordered. The loop (LOOP.md) takes the first item that is not `[x]` or `[!]`.
One item = one iteration = one commit+push. "Done when" is the acceptance
gate on top of the standard gates (clippy/tests/tsc/eslint/wrong-tenant/wire).
Detail source: `docs/features.md` → Business modules. Do not reorder; do not
invent items — a discovered prerequisite becomes part of the current item if
small, or a `[!]` note for the human if large.

## Wave B1 — alo Billing (Quotes & Invoices, EU e-invoicing)

- [x] B1.01 Design note `docs/design/billing.md`: surface (routes), data model (customers, products, quotes, invoices, lines, payments), error map, tenancy, numbering approach, out-of-scope. Done when: the note answers the implement-skill's four blocks and names the rejected alternative for numbering.
- [x] B1.02 Migration + store: `billing_customers` (name, address fields, country, VAT id, email, payment terms days, currency, linked contact id nullable) with tenant-scoped CRUD on AccountStore + unit tests + wrong-tenant test. Done when: tests prove create/read/update/list/archive and cross-tenant denial.
- [x] B1.03 VAT-id format validation (VIES checksum-style per-country patterns, pure fn, unit-tested) wired into customer create/update; invalid → typed error. Done when: valid DE/FR/NL/BE/PL ids pass, malformed fail, empty allowed (B2C).
- [x] B1.04 Migration + store: `billing_products` (name, unit, unit price cents, VAT rate bp, active) CRUD + tests incl. wrong-tenant. Done when: same bar as B1.02.
- [x] B1.05 HTTP: `/billing/customers` + `/billing/products` CRUD routes (auth, Problem errors, validation) + wire-verify with curl against local backend (create→list→update→archive, 401 without token, 422 bad VAT id). Done when: the curl transcript in STATE.md shows all codes.
- [x] B1.06 Migration + store: `billing_invoices` + `billing_invoice_lines` (description, qty milli-units, unit price cents, VAT rate bp, line order); status enum draft/issued/paid/void; server-side totals fn (net, VAT per rate, gross) as pure code with property tests (line sums always equal totals; no float anywhere). Done when: property tests + wrong-tenant pass.
- [x] B1.07 Store: draft-invoice lifecycle — create/update lines while draft only; typed error editing a non-draft. Done when: tests prove immutability after issue-marker set.
- [x] B1.08 Store: ISSUE flow — per-tenant gapless sequence (`billing_sequences` row-locked in the same tx), number format `INV-YYYY-NNNNN`, issue sets number+issue date+due date and freezes the invoice. Concurrency test: two parallel issues never share or skip a number. Done when: the concurrency test passes 100 iterations.
- [x] B1.09 Store: credit notes — negative invoice referencing original (same numbering sequence, type flag); original must be issued; totals mirror. + tests. Done when: crediting a draft fails typed; ledger of original+credit sums to zero.
- [x] B1.10 HTTP: `/billing/invoices` routes — CRUD drafts, POST issue, POST credit-note, list with status filter + overdue computed. Wire-verify the full arc: draft→issue (number assigned)→credit. Done when: transcript in STATE.md.
- [x] B1.11 Migration + store: `billing_quotes` + lines (same line model, shared code where clean); lifecycle draft/sent/accepted/declined/expired with allowed-transition tests. Done when: invalid transitions fail typed.
- [x] B1.12 Store+HTTP: accept-quote → creates a draft invoice copying lines (link back to quote); wire-verified. Done when: accepted quote yields an editable draft invoice with identical totals.
- [x] B1.13 Web: Billing module skeleton — rail entry (workspace surface only), routes `/billing`, list pages for customers + products with create/edit dialogs, i18n en. Done when: build clean; CRUD works against local backend in the browser-facing API (curl-verifiable endpoints already proven).
- [x] B1.14 Web: invoice list (status chips, overdue highlight) + draft editor (customer picker, line rows with product picker or free text, live totals from server response — the client never computes money). Done when: tsc/eslint/build clean; totals shown come from the API.
- [x] B1.15 Web: issue flow UI (confirm dialog shows "this assigns number and freezes"), issued view read-only, credit-note button; quote pages mirroring invoices incl. accept→invoice. Done when: build clean; STATE.md notes the manual click-path exercised against local backend.
- [x] B1.16 Invoice/quote HTML print view: a clean branded document (tenant name/logo placeholder, addresses, lines, VAT breakdown, payment terms, bank details from tenant settings) rendered as a print-optimised page (this is also the PDF source). Done when: print CSS yields a correct one-page A4 in headless Chrome check.
- [x] B1.17 Server-side PDF: render the print view to PDF via headless chromium in a build-time-pinned container OR a pure-Rust HTML-to-PDF path — design decision recorded in billing.md first; endpoint `GET /billing/invoices/:id/pdf`. Done when: curl saves a valid PDF (magic bytes + non-trivial size) for an issued invoice; 404 foreign-tenant.
- [x] B1.18 Send via alo Mail: POST `/billing/invoices/:id/send` drafts an email to the customer with the PDF attached using the existing draft machinery (never auto-sends; lands in user Drafts for review, consistent with agent send rules). Done when: the draft with attachment exists in Drafts, wire-verified.
- [x] B1.19 Payments: `billing_payments` (invoice id, date, amount cents, method, reference) + partial payments; invoice derived state paid/partially-paid; overdue view (issued+due<today+unpaid). + tests + routes + UI list. Done when: partial then full payment flips states correctly on the wire.
- [x] B1.20 VAT summary per period: store query aggregating issued invoices by VAT rate for a date range + `/billing/reports/vat?from&to` + CSV export + minimal UI. Done when: a seeded quarter reproduces hand-computed totals exactly.
- [x] B1.21 Multi-currency: currency on customer/invoice, ECB reference-rate table (manual seed + daily-rate import format parser, no external calls in the loop), stored rate snapshot at issue. Done when: a EUR-tenant invoice in USD stores rate + EUR equivalents; VAT report converts correctly.
- [x] B1.22 ★ Factur-X: generate EN 16931 (profile EN 16931) CII XML from an issued invoice, embed into the PDF as PDF/A-3 attachment; golden-file tests against the official sample set. Done when: our XML validates against the EN 16931 schematron in the test suite.
- [x] B1.23 ★ XRechnung: UBL 2.1 output for the same invoice model (`GET .../xrechnung.xml`), validated by the XRechnung schematron in tests. Done when: schematron-clean for the golden invoices.
- [x] B1.24 E-invoice receiving: parse inbound Factur-X/XRechnung (from an uploaded file first) into a `billing_bills` record (supplier, lines, totals, due) for approval; malformed → typed 422. Done when: the official samples import; totals match.
- [x] B1.25 ★ Billing agent tools (ADR 0034): `create_invoice_draft`, `quote_to_invoice`, `draft_payment_reminder` in the agent allowlist + executors reusing B1 store fns; propose-then-approve; source-resolution for "invoice X" by number; structural wire-verify (no model calls). Done when: execute paths verified with curl like the Mail agent's were.
- [x] B1.26 Dunning (manual): reminder email drafts per overdue invoice (template with days-overdue), one click from the overdue view → Drafts. Done when: draft content correct on the wire.
- [x] B1.27 Wave review: fr/nl translations for all billing strings; CHANGELOG sweep; docs/design/billing.md updated to as-built; ROADMAP B1 boxes ticked; note remaining human items (Peppol AP account, Caddyfile prefix, deploy). Done when: no [B1] feature in features.md is silently missing — each is shipped or listed as a cut with a reason.

## Wave B2 — alo CRM (+ billing extensions)

- [x] B2.01 Design note `docs/design/crm.md` (deals model, thread-linking approach, pipeline stages, tenancy) same bar as B1.01.
- [x] B2.02 Migration + store: `crm_pipelines` (per-team, default seeded) + `crm_stages` (ordered, win/loss flags) + CRUD + tests incl. wrong-tenant.
- [x] B2.03 Migration + store: `crm_deals` (title, customer/contact link, value cents, currency, expected close, stage, owner, source, lost reason nullable) + stage-move with history rows + tests.
- [x] B2.04 HTTP `/crm/*` routes for pipelines/stages/deals + wire transcript.
- [x] B2.05 ★ Deal↔mail linking: store table deal_threads; suggest-by-domain (pure fn over message from-addrs) requiring user confirm; routes; tests prove another tenant's thread can never be linked.
- [x] B2.06 Activities on a deal: notes + next-step (creates a real Task via existing tasks store, linked); shows due in deal. Tests + routes.
- [x] B2.07 Web: pipeline kanban (reuse Tasks board interaction), deal drawer (value, stage, activities, linked threads with open-in-mail), list view + filters. Build clean; manual path in STATE.md.
- [x] B2.08 Win/loss: closing flow (won → optional link to create quote/invoice B1; lost → reason picker), simple per-pipeline value-by-stage report + CSV.
- [x] B2.09 CSV/Excel lead import with mapping preview + dedupe by email domain; import report. Wire-verified with a fixture file.
- [x] B2.10 ★ CRM agent tools: `create_deal` (incl. from thread source), `move_deal_stage`, `draft_followup` — allowlist + executors + structural verify.
- [x] B2.11 Billing extension — recurring invoices: schedule table, due-run creates DRAFTS (never auto-issues), UI badge; time-based test with injected clock.
- [x] B2.12 Billing extension — SEPA pain.001 export for approved bills (from B1.24) with schema-valid XML golden tests.
- [x] B2.13 Audit log (cross-cutting): append-only record of create/update/status events for billing+crm entities, `GET /audit?entity=`, UI tab on records. Tests: every mutating route writes exactly one entry.
- [x] B2.14 Wave review: fr/nl, CHANGELOG, design docs as-built, features.md [B2] reconciliation.

## Wave BI-1 — alo Insights, first slice (ADR 0037; inserted by owner decision 2026-08-07)

- [x] BI1.01 Design note `docs/design/insights.md`: ChartSpec model (typed measure/dimension/period/filter envelope — the AI never writes SQL), the whitelisted semantic layer over billing+crm views, tile/dashboard model, tenancy, chart-library choice (embedded Apache-2.0 lib, ADR 0033 precedent), out-of-scope. Same four-block bar as B1.01.
- [x] BI1.02 Migration + store: `insight_dashboards` + `insight_tiles` (typed spec JSON validated on write like site sections; layout order) tenant-scoped CRUD + wrong-tenant tests.
- [x] BI1.03 Query engine: ChartSpec → safe SQL against whitelisted billing/crm views only; every query tenant-bound by construction; golden tests on a seeded tenant reproduce hand-computed series; a test proves a foreign tenant's spec yields only their own data.
- [x] BI1.04 HTTP `/insights/*`: dashboards/tiles CRUD + `POST /insights/eval` (spec → series) + wire transcript incl. 401/422; ALSO add `/insights` to the vite dev proxy list (the S1.11 lesson).
- [x] BI1.05 Web: Insights rail tab (workspace surface), dashboard grid, tile renderers (number, bar, line, pie, table) via the chosen embedded chart lib under alo chrome; i18n en.
- [x] BI1.06 Gallery + the ★ zero-setup "Business overview": prebuilt specs (Billing: revenue by month, outstanding, overdue aging, VAT by period; CRM: pipeline by stage, won this month, win rate) and the default dashboard auto-built per tenant on first visit. Done when: a seeded tenant opens Insights and sees live numbers with zero clicks.
- [x] BI1.07 ★ Ask-to-chart: alo-ai NL→ChartSpec envelope (strict parse + one repair retry, fixture tests, NO live calls) + propose-then-approve UI (chart preview → Approve pins to dashboard).
- [x] BI1.08 Wave review: fr/nl strings, CHANGELOG, design as-built, features [BI-1] reconciliation.

## Wave B3 — alo Projects & Timesheets

- [x] B3.01 Design note `docs/design/projects.md` (client-project typing over existing task projects, time model, approval, rates).
- [x] B3.02 Store: client-project extension (customer link, budget hours/cents, hourly rate) on existing task projects + tests.
- [x] B3.03 Migration + store: `time_entries` (user, project, task nullable, started/minutes, billable, note, rate snapshot cents) + CRUD + tests incl. wrong-tenant.
- [x] B3.04 Timer routes: start/stop (one running per user, stop writes entry) + manual entry + weekly list; wire transcript.
- [x] B3.05 Approval: weekly submit → manager approve/reject; approved entries lock (edit → typed error). Tests for the lock.
- [x] B3.06 ★ Billable → invoice: select approved unbilled entries for a customer → invoice draft lines (B1), entries marked billed with invoice link; unbilled view. Wire-verified arc.
- [x] B3.07 Web: timer widget in shell (workspace surface), timesheet week grid, project budget bar, approvals inbox page for managers.
- [x] B3.08 Project profitability report (hours×rates vs budget) + CSV.
- [ ] B3.09a Milestones: model + store + timeline rendering over existing boards; tests.
- [ ] B3.09b Project templates: create-from-template copying boards/milestones; tests + wire.
- [ ] B3.10a ★ Projects agent, answers+time: `log_time` (draft) + `project_status_summary` (answer from sources) in the allowlist + executors; structural verify.
- [ ] B3.10b ★ Projects agent, calendar: `draft_timesheet_from_calendar` (drafts entries from Agenda events for approval); structural verify.
- [ ] B3.11 Wave review: fr/nl, CHANGELOG, design as-built, features [B3] reconciliation.

## Wave B4 — alo Finance (Expenses & Accounting core)

- [ ] B4.01 Design note `docs/design/finance.md` — CoA model, journal invariants, posting rules per document type, reconciliation model, period locking. The debits==credits invariant stated as a property test plan.
- [ ] B4.02 Migration + store: chart of accounts (code, name, type asset/liability/equity/income/expense, EU-SME default seed per tenant) + CRUD (custom accounts) + tests.
- [ ] B4.03a Journal tables + balanced-entry enforcement: `fin_entries` + `fin_postings`, unbalanced rejected in the tx; basic insert/read tests + wrong-tenant.
- [ ] B4.03b Journal property tests: random generated documents always balance; posting-query API for later reports.
- [ ] B4.04a Auto-posting, invoices: issued invoice → AR/revenue/VAT postings; golden test vs hand-written entries.
- [ ] B4.04b Auto-posting, payments: payment → bank/AR postings incl. partials; goldens.
- [ ] B4.04c Auto-posting, credit notes: full reversal postings; goldens; ledger of original+credit sums to zero.
- [ ] B4.05a Expenses model: migration + store CRUD (category→account map, project link, method, receipt file ref) + wrong-tenant tests.
- [ ] B4.05b Expense approval flow: submit/approve/reimburse transitions + routes + wire transcript.
- [ ] B4.06a ★ Receipt parsing core: deterministic extractor (vendor/date/amounts/VAT) behind a pluggable trait; fixture receipts prove it; AI backend seam flagged for human wiring.
- [ ] B4.06b Receipt confirm path: upload route + parsed-fields-for-confirmation response + confirmed→expense creation; wire transcript.
- [ ] B4.07 Mileage claims (per-km rate table per tenant, entry → expense).
- [ ] B4.08a Bank import: CAMT.053 parser (golden files from public samples) → staged `bank_lines`; malformed → typed errors.
- [ ] B4.08b Bank import: MT940 parser, same contract + goldens.
- [ ] B4.08c Bank import: CSV mapping wizard model + staging + partial-import report; routes + wire.
- [ ] B4.09a ★ Reconciliation, exact stage: amount+reference matcher; confirm → payment/expense postings; precision tests + no cross-tenant leakage.
- [ ] B4.09b Reconciliation heuristics: windowed matching + per-tenant learned-rules table; fixture precision tests.
- [ ] B4.09c Manual matching: unmatched-line model + match/unmatch routes; wire transcript.
- [ ] B4.10 Fiscal periods + soft close (postings before lock date → typed error; admin unlock audited).
- [ ] B4.11a Report: P&L — store query + route + CSV; golden on the seeded year.
- [ ] B4.11b Report: balance sheet — same contract + goldens.
- [ ] B4.11c Report: aged receivables/payables — same contract + goldens.
- [ ] B4.11d Report: VAT-return figures — same contract + goldens.
- [ ] B4.12 Accountant role: scoped access (finance read + journal write only, no mail/files) via Spaces/roles; tests prove the scope.
- [ ] B4.13a Web finance: module skeleton + expenses flow (submit/approve/reimburse screens).
- [ ] B4.13b Web finance: bank import + reconciliation screen.
- [ ] B4.13c Web finance: CoA editor + the four report pages with CSV buttons.
- [ ] B4.14a ★ Finance agent, categorise: `categorise_transactions` (draft) — allowlist + executor + structural verify.
- [ ] B4.14b ★ Finance agent, answers: `vat_summary` + `flag_anomalies` (answers with citations); structural verify.
- [ ] B4.15 Wave review: fr/nl, CHANGELOG, design as-built, features [B4] reconciliation.

## Wave B5 — alo Inventory (Purchasing, Stock, Orders)

- [ ] B5.01 Design note `docs/design/inventory.md` — product/stock/move model (moves-only, no in-place quantity edits), locations, PO/SO state machines.
- [ ] B5.02 Migration + store: product catalog upgrade (SKU, barcode, stocked-vs-service, purchase price, photos via Drive) building on B1.04 + tests.
- [ ] B5.03 Suppliers (+ per-supplier price/lead time) + tests.
- [ ] B5.04a Locations + stock moves: `inv_moves` (from/to/qty/reason/ref), on-hand = sum with cached per-location consistency test; wrong-tenant tests.
- [ ] B5.04b Stock adjustments: manual adjustment moves with reason codes + routes + wire.
- [ ] B5.05a Purchase orders, lifecycle: model + draft→sent (PDF via alo Mail draft) + state tests + routes.
- [ ] B5.05b Purchase orders, receiving: received → stock moves + bill draft created (three-way-lite link); arc wire-verified.
- [ ] B5.06a Sales orders: model + order→delivery note (stock moves out) + routes + tests.
- [ ] B5.06b Sales orders → invoice: delivery→invoice draft (B1 bridge); full arc wire-verified.
- [ ] B5.07 Reorder rules (min/target per product/location) + shortage query feeding agent proposals.
- [ ] B5.08a Stocktake, counting: count-sheet snapshot + variance list; tests.
- [ ] B5.08b Stocktake, applying: variance → adjustment batch (B5.04b moves); wire.
- [ ] B5.09a Web inventory: catalog + stock-by-location screens.
- [ ] B5.09b Web inventory: PO + SO flow screens.
- [ ] B5.09c Web inventory: barcode scan input (camera + keyboard-wedge fallback).
- [ ] B5.10 ★ Inventory agent tools: `reorder_proposals` (draft POs), `stock_answer` — structural verify.
- [ ] B5.11 Wave review: fr/nl, CHANGELOG, design as-built, features [B5] reconciliation.

## Wave B6 — alo HR

- [ ] B6.01 Design note `docs/design/hr.md` — employee model, leave policies/balances, approvals, recruitment pipeline; explicit EU AI Act posture (screening = suggest-only + logged human decision).
- [ ] B6.02a Employees, records: migration + store (person data, role, team, manager, linked user) + HR-role access scoping tests.
- [ ] B6.02b Employees, org + documents: org chart from manager links + contract PDFs in Drive under HR-only permissions; routes + wire.
- [ ] B6.03a Leave, the math: policies (annual/sick/unpaid, accrual per year) + balance computation property-tested.
- [ ] B6.03b Leave, the flow: request→manager approval + balances applied + team absence feed into Agenda; routes + wire.
- [ ] B6.04 Public-holiday calendars per country (seed data + per-tenant selection) affecting balance math.
- [ ] B6.05 Onboarding/offboarding checklists (template → instance per employee, ties to admin account creation as manual steps).
- [ ] B6.06a Recruitment, model: openings + applicants (CV in Drive, notes, stages) + routes + scoping tests.
- [ ] B6.06b Recruitment, board: applicant pipeline board UI on the shared board pattern.
- [ ] B6.07 Approvals inbox: one manager view unifying leave/expenses/timesheets (B3/B4 hooks) with counts.
- [ ] B6.08a Web HR: directory + org chart.
- [ ] B6.08b Web HR: leave request/approve screens + absence calendar.
- [ ] B6.08c Web HR: recruitment board screen + approvals-inbox integration.
- [ ] B6.09 ★ HR agent tools: `who_is_off` (answer), `draft_letter_from_template` (draft) — screening explicitly absent per design note; structural verify.
- [ ] B6.10 Payroll export: per-period CSV of salary-relevant data (no calculation) with a per-country column mapping config.
- [ ] B6.11 Wave review: fr/nl, CHANGELOG, design as-built, features [B6] reconciliation.
- [ ] B6.12a FINAL arc, money: quote→invoice→payment→ledger and deal→won→invoice, wire-verified end-to-end on local; transcript in STATE.md.
- [ ] B6.12b FINAL arc, operations: hours→invoice line, PO→receive→bill→reconcile, leave→Agenda — transcripts; then `LOOP COMPLETE`.
