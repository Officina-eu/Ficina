# Design note — alo Insights (ChartSpec, the semantic layer, dashboards)

Status: designing · 2026-08 · ADR 0037 · Business track wave BI-1

alo Insights is the cross-module analytics surface: a top-level tab where a
business sees the numbers of all its processes, pre-built from day one, with
no connector, no ETL and no data person — because every module already
writes to one tenant-scoped Postgres. Wave BI-1 is the first slice: the
query engine, the Insights tab, a gallery of ready-made tiles over Billing
and CRM, the zero-setup **Business overview** dashboard, and ask-to-chart.

This note records the decisions before the first migration lands: what a
**ChartSpec** is, what the **semantic layer** whitelists, where money is
allowed to be added up, how a tile and a dashboard are stored, the error
map, the tenancy story, the chart library, and the cuts. It is updated to
as-built at the BI-1 wave review (BI1.08).

The one sentence the rest of the note serves: **the AI never writes SQL, and
neither does the user — a chart is a typed envelope over a closed catalog,
compiled by us, bound to one tenant by construction.**

## Surface

- **Inputs:** authenticated workspace users driving `/insights/*` on
  `alo-jmap` — dashboard and tile CRUD, tile reordering, evaluating a spec
  (saved or ad-hoc), reading the catalog and the gallery, and the
  natural-language ask that *proposes* a spec (BI1.07, fixture-verified in
  the loop — no live model calls).
- **Outputs:** JSON **series** — integer values with a declared unit
  (money in cents + currency, counts, percentages in basis points),
  ISO bucket keys, and catalog label ids the client translates. Plus the
  catalog itself (which datasets, measures, dimensions and filters exist)
  and the stored dashboards/tiles.
- **Who calls it:** the web module `web/src/insights` (the Insights tab, the
  tile builder, the ask-to-chart approval card). `platform/alo-ai`'s
  insights module produces the proposed ChartSpec envelope that `alo-jmap`
  validates and evaluates. Nothing else reads these routes; there is no
  public surface and no export endpoint in BI-1.

### Routes

All under one new top-level prefix, `/insights` (RFC 9457 `Problem` bodies,
`authenticate`d, registered in `server.rs` like every other module).

| Route | Does |
|---|---|
| `GET /insights/catalog` | the whitelisted datasets, measures, dimensions, filters, allowed pairings, and the gallery entries — the single source of truth the builder UI offers from |
| `GET /insights/dashboards` | the tenant's dashboards (seeding the Business overview on first read, BI1.06) |
| `POST /insights/dashboards` | create |
| `GET /insights/dashboards/:id` | one dashboard with its tiles, in layout order |
| `PATCH /insights/dashboards/:id` | rename / reorder tiles |
| `DELETE /insights/dashboards/:id` | delete (tiles cascade) |
| `POST /insights/dashboards/:id/tiles` | pin a spec as a tile |
| `PATCH /insights/tiles/:id` | retitle / replace spec / move |
| `DELETE /insights/tiles/:id` | unpin |
| `GET /insights/tiles/:id/data` | evaluate a stored tile |
| `POST /insights/eval` | evaluate an ad-hoc spec — the builder's live preview; stores nothing |
| `POST /insights/ask` | natural language → a **proposed** ChartSpec + its preview series; stores nothing (BI1.07) |

`/insights` is a new top-level prefix, so two things follow it around: the
**vite dev proxy** `API_PATHS` list in `web/vite.config.ts` gains `/insights`
in the same item that adds the routes (the S1.11 lesson — a missing prefix
makes every dev-mode call return the SPA's HTML), and the **production
Caddyfile** needs the prefix at the next deploy, a human action recorded in
STATE beside `/billing`, `/crm` and `/audit`.

### Web surface

`web/src/insights/` — the Insights rail entry (workspace surface only, like
Billing and CRM), a dashboard grid of tiles, five tile renderers (number,
bar, line, pie, table), the tile builder (dataset → measure → dimension →
period → filters, every option from `/insights/catalog`), and the
ask-to-chart card: preview, **Approve** pins it to a dashboard, discard
leaves nothing behind (ADR 0034). All strings through `i18n/en.ts`; fr/nl at
the wave review.

## The ChartSpec

A ChartSpec is the whole contract between a user (or a model) and the query
engine. It is a typed Rust value with a serde representation, stored as
JSONB on a tile and accepted on the wire, and it is **validated on write**
exactly the way a site page's sections are (`site_model`, ADR 0036):

```jsonc
{
  "schema_version": 1,
  "dataset":   "billing.documents",          // enum variant, not a table name
  "measure":   { "id": "net", "agg": "sum" },// enum variant per dataset
  "dimension": { "id": "issue_date", "grain": "month" },  // or a field dim, or none
  "period":    { "kind": "last_n", "n": 12, "grain": "month" },
  "filters":   [ { "id": "status", "op": "in", "values": ["issued", "paid"] } ],
  "sort":      { "by": "dimension", "dir": "asc" },
  "limit":     24,
  "viz":       "bar"
}
```

Four properties make it safe, and each is a rule the code enforces rather
than a convention:

1. **Nothing in a spec is an identifier.** `dataset`, `measure.id`,
   `dimension.id` and `filters[].id` are enum variants; each maps to a
   `&'static str` SQL fragment written by us at compile time. No string from
   a request or a model ever reaches a query as SQL text. The only
   caller-controlled values are *bound parameters* (dates, ids, integers,
   enum-checked strings).
2. **Unknown is a refusal, not a default.** The serde types are
   `deny_unknown_fields`; an unknown dataset, measure, dimension, filter, op
   or viz is a `422` naming the field. A model that invents
   `"measure": "profit"` gets a typed error, never an empty chart.
3. **Pairings are declared, not assumed.** The catalog holds a compatibility
   matrix — which measures a dataset offers, which dimensions each measure
   may be broken down by, which grains a time dimension allows, which filters
   apply. `sum(deal value)` by `vat_rate` is not an odd chart, it is a `422`.
4. **Bounds are part of the type.** `limit` ≤ 50 categories, ≤ 400 time
   buckets, a period window ≤ 5 years, ≤ 8 filters, each filter ≤ 25 values.
   Over a bound is a `422` that says which bound and what the maximum is —
   the shape the billing store already uses for line counts.

**Versioning.** The envelope carries `schema_version`, and a bump ships a
pure upgrade function applied on read, with the stored JSON rewritten lazily
on the next save — the site-sections pattern, for the same reason: a tile
saved by a newer client must not break an older reader mid-deploy. On read,
a tile whose spec fails to parse is returned **marked unreadable**, and the
rest of the dashboard renders; a dashboard never 500s because one tile is
from the future.

## The semantic layer

The catalog is the whitelist. BI-1 ships four datasets, all over tables that
already exist (`0100`–`0118`):

| Dataset | Rows | Measures | Dimensions | Filters |
|---|---|---|---|---|
| `billing.documents` | invoices + credit notes that stand (`status IN ('issued','paid')`) | `net`, `vat`, `gross`, `count` | `issue_date` (day/week/month/quarter/year), `customer`, `currency`, `vat_rate`, `status` | `customer`, `currency`, `status`, `vat_rate` |
| `billing.receivables` | issued/unpaid documents as of a date | `outstanding`, `count` | `age_bucket` (not due / 0–30 / 31–60 / 61–90 / 90+), `customer`, `due_date`, `currency` | `customer`, `currency` |
| `billing.payments` | recorded payments | `amount`, `count` | `paid_on` (day…year), `method`, `customer`, `currency` | `customer`, `method`, `currency` |
| `crm.deals` | deals, open and closed | `value`, `count`, `win_rate` | `stage`, `owner`, `source`, `outcome`, `created_at`/`closed_at`/`expected_close` (month…year), `currency` | `pipeline`, `owner`, `outcome`, `currency` |

A dataset is a **logical view**, not a database view. It is a Rust row-query
in one module plus the catalog entry describing it; that is deliberate, and
it is the note's second decision.

**Rejected: Postgres views as the semantic layer.** A view cannot carry the
tenant predicate by construction — that would need row-level security, which
alo does not use; the tenancy of every read would move from the store handle
(where it is structural today) into a policy we would have to keep correct
forever. Worse, the rules that make a figure *right* — only documents that
stand are counted, credit notes subtract, each document's own rounded VAT is
summed, a document converts at its own frozen rate — already live in Rust
(`billing_vat_report`, `billing_totals`, `billing_fx`). A view would have to
restate them in SQL, and the first cent of disagreement between a tile and
the VAT return is a defect that destroys trust in both. Datasets in Rust
reuse those functions instead of re-deriving them; changing one is a code
change with tests, not a migration.

### Where money may be added up

One law, because it is the law that keeps a tile and a tax return agreeing:

> **SQL may sum a stored integer column. SQL may never *derive* money** — it
> never multiplies quantity by price, never applies a VAT rate, never
> rounds, and never converts a currency.

So the datasets split, honestly, into two shapes:

- **Direct sums (SQL).** `crm.deals` sums `value_cents`, `billing.payments`
  sums `amount_cents` — stored, exact, bounded integers, grouped and summed
  by Postgres. This is exactly what `crm_report` already does, and the
  precedent is deliberate.
- **Folded sums (Rust).** `billing.documents` and `billing.receivables` are
  document money: net and VAT come from lines through
  `billing_totals::totals`, which rounds **once per rate subtotal, per
  document**, and the accounting-currency figure comes from the document's
  own `FxSnapshot` via `billing_fx::restated`. The SQL reads line figures
  and the group key; the fold happens in Rust, in the same functions the
  printed invoice and the VAT report use.

Two consequences the note states out loud:

- **Row-bounded, not unbounded.** A folded dataset reads line rows, so every
  such query is bounded by its period, by the catalog's filters and by a hard
  row cap (200 000 line rows). Over the cap is a typed `422` asking for a
  narrower period — never a silent truncation, and never a query that ties up
  a connection. At SME scale (the tenants ADR 0035 describes) a five-year
  window is thousands of rows, not millions.
- **Unconverted documents are reported, never dropped.** A document with no
  usable rate snapshot is counted in a `notes` entry on the series
  (`unconverted_documents: n`), the same honesty rule `billing_vat_report`
  applies to a VAT return: a figure is never part-invented, and the tile says
  when part of the period could not be restated.

**Which date a period narrows on** is the one thing a spec has to be able to
say out loud, so BI1.03 added an optional `period_on` to the envelope. Three
rules, in order: what `period_on` names; failing that the chart's own time
breakdown (revenue by month over the last year narrows on the month it draws);
failing that the dataset's declared default — issue date, due date, the day
the money arrived, and for a deal the day it was raised. "Won this month" is
about the day a deal *closed*, and it says so; without the field it would have
had to mean "raised this month and since won", which is a different sentence.

**Deals are never converted, and neither are payments.** `crm.deals` measures
group by currency and stop there — a forecast has no tax point, so there is no
honest rate to convert it at (`docs/design/crm.md` § The pipeline report never
converts currencies). `billing.payments` stops there for the same kind of
reason, decided at BI1.03: the rate frozen on an invoice is the rate of its
**tax point**, not of the day the money arrived, and restating cash at it would
report a figure no bank statement agrees with. Both render one series per
currency; neither adds euros to dollars behind a single bar.

### The series that comes back

```jsonc
{
  "unit":   { "kind": "money", "currency": "EUR" },   // or count | percent_bp
  "series": [ { "key": "EUR", "label": { "kind": "raw", "text": "EUR" },
                "points": [ { "bucket": "2026-01", "value": 1234567 } ] } ],
  "notes":  [ { "code": "unconverted_documents", "count": 3 } ],
  "truncated": false
}
```

- **Values are integers.** Money in cents, counts as counts, ratios in basis
  points (the win rate, exactly as `crm_report::win_rate_bp` already gives
  it). No float carries a figure a person reads.
- **The client never computes money** — the same law Billing's UI runs under.
  The browser formats cents into a locale string and draws a bar; it does not
  sum, subtract, prorate or convert.
- **Labels are ids or user data, never English from the server.** A catalog
  label (`"invoices"`, `"31–60 days"`) crosses as `{ "kind": "catalog", "id":
  … }` and the client translates it; a customer name or a stage name crosses
  as `{ "kind": "raw", … }` because it is the tenant's own words. A VAT rate is
  neither, so BI1.03 added a third kind, `{ "kind": "rate_bp", "bp": 2100 }` —
  a number the client formats, rather than a percent sign we picked. Buckets
  are ISO strings (`2026-01`, `2026-Q1`, `2026-W03`, `2026-01-15`) formatted
  per locale in the client, and carry no label at all. Hardcoded English in a
  European product is a bug, and a chart axis is not an exception.
- **A quiet bucket is a zero; an unanswered one is absent.** A month inside a
  bounded window that earned nothing comes back as `0` rather than as a gap. A
  *ratio* does not: a month in which nothing closed has no win rate, and
  printing 0 % there would be a fact nobody stated
  (`crm_report::win_rate_bp` makes the same distinction).
- **Top-N with a remainder.** A category dimension over the limit folds the
  tail into one `other` point flagged as such — never a chart that silently
  omits rows.

## Dashboards and tiles

Migration `0119_insight_dashboards.sql` (the business track's `01xx` block;
`0118` is the last one in `main`).

- **`insight_dashboards`** — tenant, id, name, `system_key` (NULL for a
  user-made board; `'business_overview'` for the seeded one, with a partial
  unique index on `(tenant_id, system_key)` so the seed is idempotent and
  race-free), `created_by`, timestamps. A seeded dashboard is an **ordinary
  dashboard from the moment it exists**: it can be renamed, its tiles
  removed, tiles added. The key exists only so the seed runs once.
- **`insight_tiles`** — tenant, id, dashboard ref (composite FK, tenant-pinned
  like every other business table, `ON DELETE CASCADE`), title, `spec` JSONB
  validated against the typed ChartSpec on write, `viz`, `position` (fractional
  ordering, the ADR 0022 board shape — an ordering, never a quantity), `span`
  (1–4 grid columns), timestamps. Caps: 40 tiles per dashboard, 30 dashboards
  per tenant, 8 KB per spec.

**Nothing computed is stored.** There is no results table, no snapshot, no
cache in BI-1 — every tile evaluates from the documents each time, for the
reason the VAT report gives: a stored subtotal can outlive the rows that
justified it, and a number that disagrees with the record underneath it is
worse than a slow number. Caching is a BI-2 question with an invalidation
design behind it.

**The Business overview (BI1.06)** is a set of prebuilt specs in Rust —
revenue by month, outstanding, overdue aging, VAT by period, pipeline by
stage, won this month, win rate — materialised into real dashboard and tile
rows the first time a tenant opens Insights, inside one transaction guarded
by the `system_key` index. *Rejected: rendering the overview virtually from
code on every visit.* It would be a second kind of dashboard that cannot be
edited, reordered or extended, and the first user request would be to change
one tile on it. Seeding rows means there is exactly one dashboard model.

## The ask (BI1.07)

Natural language → ChartSpec, never SQL, never a query. `platform/alo-ai`
gains an `insights` module that describes the *catalog* to the model — the
same closed vocabulary the builder UI offers — and asks for one ChartSpec
envelope. Strict schema parse; on failure, exactly **one repair retry**
carrying the validation error; a second failure is a typed refusal the UI
shows as "couldn't build a chart from that", with nothing changed and
nothing pinned. Propose-then-approve (ADR 0034): the response is a preview
the user sees rendered, and **Approve** is what writes a tile.

The loop verifies this **structurally only** — fixture model outputs through
the parser and the compiler, routes present, `401` unauthenticated, `422` on
a spec that fails validation, and a real evaluation against the local
database. No live model call is made unattended, and an unconfigured AI key
degrades to the manual builder, which is the whole surface minus the ask.

## Chart rendering

**Apache ECharts (Apache-2.0), embedded as a library, imported tree-shaken.**
It is the Apache-2.0 chart library ADR 0037 names, it needs no network at
runtime (no CDN, no tiles, no telemetry — we bundle it and use canvas
rendering), it covers all five BI-1 viz types plus the ones BI-2 wants, and
it is a *library under our chrome*, the ADR 0033 precedent that Univer and
BlockNote already set. Its licence sits comfortably under our AGPL-3.0 core.

One rule keeps it a dependency rather than an architecture: **exactly one
file imports `echarts`** — `web/src/insights/chart/`, which takes a series
and an alo theme and returns a chart. Tile renderers talk to that wrapper, so
swapping the engine is a one-file change and no chart library's types leak
into the module. Only the chart types we use are imported, and geo/map
components — the bulk of ECharts' weight — never are.

*Rejected: Recharts or Chart.js.* Both are fine libraries; Recharts renders
every point through React's reconciler (poor at the table-and-line sizes a
five-year daily series produces) and Chart.js needs plugins for the table and
number-tile cases. Neither is Apache-2.0, which is what the ADR asked for.
*Rejected, harder: drawing SVG charts ourselves.* Axis ticks, label
collision, stacking, legends and accessibility are a research project the
product does not need — ADR 0037 says never a from-scratch chart engine.

*Rejected: an embedded BI engine (Metabase, Superset, Cube).* A second server
to operate, in a third language, with its own connection pool and its own
notion of tenancy grafted on — three doctrine violations at once, and ADR
0037's non-goal in one line: never a separate BI server.

## Errors

RFC 9457 `Problem` bodies, the same map every business module uses.

- Unauthenticated → `401`.
- A dashboard or tile id of another tenant (or a nonexistent one) → `404`.
  Wrong-tenant is indistinguishable from nonexistent: the account-door
  pattern, unchanged.
- Spec validation → `422` with the offending field: unknown dataset /
  measure / dimension / filter / op / viz; an incompatible pairing; a grain
  the dimension does not allow; a bound exceeded (limit, buckets, window,
  filters, values, spec bytes); a malformed filter value (a date that is not
  a date, an id that is not this tenant's).
- A filter naming a customer, pipeline or owner that does not resolve **in
  this tenant** → `422`, not an empty chart: a silently empty tile is how a
  business believes it billed nothing last quarter.
- A folded query over the row cap → `422 period_too_wide`, stating the cap
  and the period that would fit.
- Dashboard/tile caps exceeded → `422`.
- The ask failing schema parse after its one repair → typed error, surfaced
  as "couldn't build a chart"; nothing stored.
- A stored tile whose spec no longer parses → **not** an error: the tile is
  returned marked unreadable and the dashboard renders.

Store errors keep the shape B1 and B2 established (`StoreError::NotFound` →
`404`, `Invalid` → `422`, `Conflict` → `409`), mapped once at the route edge.

## Tenancy

- Every `insight_*` table carries `tenant_id`; every read and write goes
  through the account door (`AccountStore`), so the tenant predicate is on
  the statement rather than in a filter someone can forget. Wrong-tenant
  reads return `NotFound`/empty — never data, never a 500. The wrong-tenant
  test is mandatory on both new store modules.
- **The query engine's tenancy is structural, twice over.** The tenant id is
  taken from the handle, never from the spec — a ChartSpec has no field that
  could name a tenant, which is why the model is never in a position to leak
  one. Every dataset fragment is written with its tenant predicate, and a
  unit test walks the *whole catalog*, compiles every dataset × measure ×
  dimension combination, and asserts each generated statement binds
  `tenant_id` as its first parameter. That test grows with the catalog by
  construction, which is the point: a dataset added without a tenant
  predicate cannot pass CI.
- On top of the structural test, the live wrong-tenant test (BI1.03):
  seed two tenants with different figures, evaluate tenant A's spec on
  tenant B's handle, and assert the numbers are B's — a spec is not a
  capability.
- Filter values that are ids are resolved **through the tenant's own store**
  before they are bound, so a guessed customer id from another tenant is a
  `422`, not a join that quietly matches nothing.
- **Dashboards are tenant-wide in BI-1**: every member of a tenant sees every
  dashboard. ADR 0037 wants Spaces-scoped sharing (finance sees finance,
  sales sees pipeline) and that is real, but it is the same cross-cutting
  role question CRM deferred (`docs/design/crm.md`) and B4.12 owns, where the
  accountant is the first scoped role. Inventing a tile-level permission
  model here would be deciding that design from its narrowest caller. The
  note says the limitation out loud rather than implying it.
- **Instrumentation carries no data.** The eval span records dataset,
  measure, dimension, grain, bucket count and duration — catalog ids and
  integers. Filter *values* (customer ids, names) are never logged, and no
  figure is. Our logs are held to the promise we sell.

## Files this wave will add

One file, one responsibility, in the shapes the store and the API already
use:

- `platform/alo-store/src/insight_spec.rs` — the ChartSpec types, validation,
  version upgrade (no SQL).
- `platform/alo-store/src/insight_catalog.rs` — datasets, measures,
  dimensions, filters, the compatibility matrix, the gallery entries (no
  SQL).
- `platform/alo-store/src/insight_query.rs` — spec → bound row reads. The
  only file in the wave that contains SQL.
- `platform/alo-store/src/insight_series.rs` — rows → series, including the
  money fold through `billing_totals` / `billing_fx`. The only file that adds
  money up.
- `platform/alo-store/src/insight_dashboards.rs`, `insight_tiles.rs` — CRUD
  and ordering.
- `platform/alo-store/src/insight_overview.rs` — the prebuilt specs and the
  idempotent per-tenant seed.
- `products/mail/alo-jmap/src/insights.rs` (dashboards/tiles),
  `insights_eval.rs` (eval + tile data), `insights_catalog.rs` (catalog +
  gallery), `insights_ask.rs` (BI1.07).
- `platform/alo-ai/src/insights.rs` — the NL → ChartSpec envelope.
- `web/src/insights/**` — the tab, the grid, five renderers, the builder, the
  ask card, and the single `chart/` wrapper around ECharts.

## Out of scope for BI-1 (cuts are decisions)

- **Tiles over modules that do not exist yet** — projects/timesheets (B3),
  finance and cash (B4), stock (B5), site analytics (S2). The catalog is
  built to grow a dataset at a time; BI-2 adds them when the tables exist.
- **Module-embedded overview strips** and the **scheduled digest mail** —
  ADR 0037 names both as the later wave.
- **Exports** (tile CSV/XLSX, PDF of a dashboard) and printing. The
  accountant's exports already exist where the figures are legal documents
  (`GET /billing/reports/vat.csv`); a chart export is BI-2.
- **Caching, materialisation, scheduled refresh.** Nothing computed is
  stored in BI-1, deliberately (above).
- **Spaces-scoped sharing and per-tile permissions** — deferred to B4.12's
  role model, with the tenant-wide limitation stated in Tenancy.
- **Period-over-period comparison, targets, trend lines, forecasting.** Each
  is a ChartSpec field or a second series, each is a real feature, and none
  is needed for the first honest picture of the business.
- **Drill-through** from a bar to the underlying invoice list, and
  cross-filtering between tiles.
- **Free-form dashboard layout** (drag-resize on a canvas). BI-1 has ordered
  tiles with a 1–4 column span, the same restraint the sites section model
  uses for the same reason: a typed layout is one an AI can also write.
- **Raw-SQL access, external connectors, and any data alo does not already
  hold** — ADR 0037's non-goals, and the product's whole claim.

## Open questions flagged for a human

- **The accounting currency of a tenant** is `billing_settings.base_currency`,
  which a tenant that has never saved settings already reads as
  `DEFAULT_CURRENCY` (EUR) rather than blank — so money tiles have a currency
  to restate into from the first visit, exactly as the VAT summary does.
  Whether Insights should *prompt* a tenant billing in several currencies to
  confirm that default before showing a restated total is a product call, not
  a code one, and is left to the wave review.
- **The ROADMAP gate on wave B2 ("B1 live with ≥1 real tenant") is still
  unmet** — B1 and B2 are code-complete but nothing is deployed, and BI-1 was
  inserted ahead of B3 by owner decision (ADR 0037). This note is exactly the
  work that belongs ahead of an unmet gate; **BI1.02 is the first item that
  writes a migration**, and the standing human actions (the `/billing`,
  `/crm`, `/audit` — and now `/insights` — Caddyfile prefixes, and a deploy)
  are unchanged.
