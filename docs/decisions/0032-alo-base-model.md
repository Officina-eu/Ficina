# ADR 0032 — alo Base: the relational data model

Status: accepted. The second alo-native type (ADR 0030) — the Airtable-class
relational base that is alo's "sheet". Built on Drive (ADR 0027); its own engine
in Rust (the two-language rule; Grist is a design reference, never a dependency).

## Context

alo's native "sheet" is not a grid of cells — it is a **relational database with
a spreadsheet face**. The value is precisely what Excel and Airtable cannot do:
typed fields, records that link to other records *and to real workspace things*
(tasks, files, people, events), and many views over the **same** records. Excel
is a compatibility type (Collabora Calc, ADR 0030) for real `.xlsx`; this ADR is
only the native relational one.

## Decision

### A Base is a Drive node backed by relational tables

An **alo Base** is a `drive_node` with `kind = 'base'` (ADR 0027) — so it lives
in one location, inherits Space membership/access, versioning of its definition,
trash, move, and copy. Unlike an alo Doc (content in a blob), a Base's data is
**relational and live** — it lives in dedicated, tenant-scoped tables, keyed back
to the Base's node. Every read and write of Base data is gated through **read/
write access to the Base's `drive_node`** (its Drive location), so a Base in a
Space is readable by members and writable by editors+, with no separate ACL.

### The model (records are the truth; views are configuration)

- **Table** — a Base contains one or more tables (`base_tables`, keyed to the
  Base node).
- **Field** — a table has typed fields (`base_fields`): `text`, `number`,
  `date`, `checkbox`, `select` (single/multi), `attachment`, `person`, and
  **`link`** (to records in another table, or to a real workspace entity). Field
  type + options define the schema.
- **Record** — a row (`base_records`). A record's cell values are stored as
  **JSONB keyed by field id** — flexible typing without an EAV cell-per-row
  explosion, the model Grist proved at scale. (Grist's *design* is the
  reference; its Python engine is never integrated.)
- **View** — a saved way to look at a table's records (`base_views`): `grid`
  (spreadsheet), `board` (kanban by a select field), `calendar` (by a date
  field), `gallery`. A view is configuration (which fields, filters, sort,
  grouping); switching view **never changes data**. This is the one-record
  principle already proven in Tasks (board and list over one row, ADR 0022) —
  the same law, generalised to arbitrary tables.

### Linked records — the alo advantage

A `link` field points at either **another table's records** in the same Base, or
a **real workspace entity** — a task, a file, a person, an event in the same
Space (the source-link fabric, ADR 0029). Airtable can do the first; only alo can
do the second, because the workspace is underneath. A linked record renders live.

### The grid UI is a permissive framework, above the waterline

The interactive multi-view surface is built on a permissively-licensed grid —
**AG Grid Community (MIT)** or **Univer (Apache-2.0)**, both TypeScript — that we
embed and own around, never a forked whole-app. The relational **engine**
(schema, records, links, and later formulas) is **ours, in Rust**, below the
waterline. Grist / Airtable / NocoDB are studied for ideas only.

### Formulas + AI (later, propose-then-approve)

Computed fields and cross-table rollups are a later slice. Natural-language
formulas, explain-and-fix-errors, and ask-your-data are AI features — and, like
all alo AI (ADR 0029), **propose-then-approve**: the AI suggests a formula or a
fix; the user commits it. Never a silent write.

### Storage sketch (tenant-scoped, cascade with the tenant)

`base_tables(tenant_id, id, node_id, name, position)`,
`base_fields(tenant_id, id, table_id, name, type, options jsonb, position)`,
`base_records(tenant_id, id, table_id, cells jsonb, position, created_at)`,
`base_views(tenant_id, id, table_id, kind, name, config jsonb, position)`.
All `REFERENCES tenants(id) ON DELETE CASCADE`; every query scoped by
`tenant_id` and gated through the Base node's Drive access. The wrong-tenant +
non-member isolation suite extends to every Base read/write, exactly as for
tasks and drive.

## Rejected alternatives

- **A cell grid (be a better Excel).** Concedes the differentiation and is the
  compatibility type's job (Collabora Calc). The native tool is relational.
- **EAV (a row per cell).** A `base_cells(record_id, field_id, value)` table
  explodes row counts and complicates every read. JSONB-per-record is the
  pragmatic, Grist-proven shape.
- **Integrate Grist/NocoDB/Baserow as a service.** Pulls a third language
  (Python) and an unowned whole-app into the tree — against the two-language and
  framework-not-whole-app rules. We build the engine; we embed only the grid UI.
- **Store a Base as a file blob (like alo Doc).** Kills the point: relational
  data must be queryable and live (links, views, rollups), not a static blob.

## Consequences

- alo Base reuses Drive for location/access/trash while owning a real relational
  engine — the genuinely new backend of the document surface.
- Views-over-one-record means board/calendar/gallery are built once over the same
  rows, never duplicating or risking data loss.
- Linked records to workspace entities are the cross-product differentiator and
  the substrate the alo Doc linked-data block (ADR 0031) renders.
- AG Grid (MIT) / Univer (Apache) license provenance is recorded for the
  confirm-before-monetizing review; the engine is ours, so no engine-license risk.
