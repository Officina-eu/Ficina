# 0035 — Business modules: alo becomes the Work OS (SAP/Odoo territory, AI-native)

Date: 2026-08-06 · Status: accepted

## Decision

alo's goal widens from "replace M365" to **the one place a business does its
work**: communication (built) plus the operational backbone SAP and Odoo sell —
billing, CRM, projects, accounting, inventory, HR. We build these **from
scratch, in Rust, on our own foundations** (alo Base's relational core, the
tenant-scoped store, Spaces permissions, the `alo-ai` agent framework), module
by module, **never as a thin clone of all of Odoo at once**.

Order of battle (waves, each shippable alone):

1. **B1 Billing** — quotes → invoices → payments, EU e-invoicing (EN 16931).
   Chosen first because EU law is forcing every SME onto structured e-invoicing
   in 2025–2027 (DE receiving already mandatory; FR/BE/PL and the ViDA rollout
   next) — a compliance-driven wedge no business can skip, and a perfect fit
   for a sovereign EU platform.
2. **B2 CRM & Sales** — deals living on real mail threads (our unfair advantage).
3. **B3 Projects & Timesheets** — extends shipped Tasks; billable hours feed B1.
4. **B4 Expenses & Accounting core** — receipts, ledger, reconciliation, VAT.
5. **B5 Purchasing & Inventory** — products, stock, purchase/sales orders.
6. **B6 HR** — employee records, leave, recruitment-lite.
Later waves (post-traction): manufacturing-lite, POS, subscriptions,
e-signature (eIDAS), marketing sends, storefront.

## AI is native, not bolted on

Every module ships **with its agent on day one** (ADR 0034 pattern): a
product-scoped tool set under propose-then-approve, access-scoped, EU-only
models. "Invoice Acme €2,400 for July consulting" must work the day invoices
exist — the agent is part of the module's definition of done, not a phase 2.
Where the EU AI Act classifies a use as high-risk (e.g. CV screening in B6),
the agent is **suggest-only with mandatory human decision**, logged.

## What we deliberately do NOT build

- **Payroll calculation** — per-country tax/social-security engines are a
  regulatory monster with zero sovereignty upside. We hold employee data and
  **export to** (or integrate with) local payroll providers.
- **Tax filing** — we produce correct, exportable VAT/therein reports; filing
  goes through the national portals or a partner.
- **Bank connections from scratch** — PSD2 access goes through a licensed
  aggregator (integrate, pinned; ADR 0009 doctrine), while CAMT/MT940/CSV
  import is built by us and works everywhere without a licence.

## Consequences

- ROADMAP gains a parallel **Business track** (waves B1–B6) that runs alongside
  Phases 3–6; features.md gains a Business modules section with the same
  tier discipline; the product doc's module map and non-goals are updated.
- Each wave passes the same gates as everything else: tenant isolation tests,
  wire verification, docs, changelog — full depth over breadth, cut scope
  never quality (CLAUDE.md).
- Migration grows a second story over time ("leave Odoo/SAP Business One"),
  starting with simple CSV/Excel imports per module — never blocking a wave.
