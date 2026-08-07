# 0037 — alo Insights: see the whole business (AI-native BI)

Date: 2026-08-07 · Status: accepted

## Decision

alo gains **alo Insights**: the cross-module analytics surface — a top-level
tab where a business sees the data of ALL its processes. Three layers:

1. **A pre-built "Business overview" dashboard that exists from day one** —
   revenue, outstanding, pipeline, hours, stock — zero setup. The unfair
   advantage: every module already writes to one tenant-scoped Postgres, so
   there are no connectors, no ETL, no "data person" — the 80% of BI-tool
   complexity that Tableau/Power BI customers pay for simply does not exist.
2. **Ask-to-chart** — plain language → chart → approve pins it (ADR 0034
   propose-then-approve). The AI NEVER writes SQL: it emits a typed
   **ChartSpec** (measure, dimension, period, filters — the same disciplined
   envelope pattern as site sections), which our engine compiles against a
   whitelisted semantic layer. Tenancy enforced by construction, not prompt.
3. **Later:** the same tiles embedded as per-module overview strips, and a
   scheduled digest mail (your numbers in your own inbox Monday morning).

Dashboards share via Spaces permissions (finance sees finance, sales sees
pipeline). Rendering: an embedded OSS chart library (Apache-2.0 class) under
our chrome, per the ADR 0033 editor precedent — we never build a chart
engine from scratch.

## Sequencing (Business track)

- **Wave BI-1** (inserted after B2 CRM): engine + Insights tab + the Billing/
  CRM gallery + the auto-built overview + ask-to-chart. Thin but real.
- **Wave BI-2** (after B4 Finance): profit/cash/aging tiles, projects+stock
  tiles, module-embedded strips, digest mail, exports.

## Non-goals

No raw-SQL access for users or models; no external data connectors (alo data
only — that IS the product); no pixel-perfect report designer (exports cover
the accountant); never a separate "BI server" to operate.
