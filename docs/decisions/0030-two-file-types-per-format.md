# ADR 0030 — Two file types per format: compatibility and alo-native

Status: accepted. The organising decision for the document surface that sits on
top of Drive (ADR 0027). Governs alo Docs, alo Sheets, and alo Slides, and the
build order for all three. Builds on ADR 0010 (Collabora as the editor engine).

## Context

"Documents" has to serve two needs that pull in opposite directions:

1. **Compatibility.** Open and edit the real `.docx` / `.xlsx` / `.pptx` files
   the rest of the world sends — with fidelity, so an offer letter or a budget
   round-trips to desktop Office without mangling. This is table stakes; getting
   it wrong loses the customer.
2. **A genuinely better native experience.** Where we are not constrained by
   Office's file format, be *great* — Notion-class documents and Airtable-class
   data, things Word and Excel structurally cannot be. This is the
   differentiation; matching Office feature-for-feature is not.

One tool cannot be both. A Word-compatible editor is shackled to the `.docx`
object model; a block/relational native tool cannot also be a faithful `.docx`
editor. Forcing one to do both yields a mediocre version of each.

## Decision

**Each format offers two file types, and the user chooses per document:**

| Format | Compatibility type | alo-native type |
|---|---|---|
| Docs   | **Word document** (`.docx`/`.odt`) — Collabora Writer | **alo Doc** — a block editor (ADR 0031) |
| Sheets | **Excel spreadsheet** (`.xlsx`/`.ods`) — Collabora Calc | **alo Base** — a relational base (ADR 0032) |
| Slides | **Presentation** (`.pptx`/`.odp`) — Collabora Impress | *(none — see below)* |

- **The compatibility types are pinned-engine documents.** They are ordinary
  files in Drive (a `.docx` is a file); they open in **Collabora** through WOPI
  (ADR 0010). Collabora is configured, never patched — a pinned upstream
  container behind our API, per doctrine. Fidelity is Collabora's job; ours is
  the integration + the surrounding app.
- **The alo-native types are ours.** alo Doc is built on **BlockNote** (MPL, a
  TypeScript block editor) — a framework we embed, not a whole app we fork. alo
  Base is our **own relational engine in Rust** (ADR 0032), with a permissive
  grid UI. Grist/Airtable/Notion are studied for ideas, never integrated as
  apps — the "framework, not whole app" rule, and the two-language rule (Rust +
  TypeScript only; Grist's Python engine is a reference, never a dependency).
- **Slides has only the compatibility type.** A native slide *canvas* is the
  hardest, least-differentiating thing to build; we do not. Slides are Collabora
  Impress documents. The differentiation for slides lives entirely in the AI
  layer (deck-from-context, brand-apply, …), applied to Impress files.

### Everything is a Drive file (ADR 0027)

Both types of every format are `drive_nodes`. They live in exactly one location
(personal or a Space) and inherit its access, versioning, and trash — a document
created in a Space is auto-shared with that Space's members, with no separate
sharing model. Native types get their own node kinds (`doc`, `base`);
compatibility types are files identified by their Office MIME type. There is no
separate "documents app" or subdomain — documents open inside the workspace.

### Creation flow

`New → Doc / Sheet / Slides` in My Files or a Space. For **Docs** and **Sheets**,
the menu offers both types plainly — "alo Doc" vs "Word document"; "alo Base" vs
"Excel spreadsheet" — defaulting to the alo-native one (the better experience),
with the compatibility one one click away. **Slides** creates an Impress
presentation directly. Upload of an existing Office file always lands as the
compatibility type.

### Build order (ADR-recorded; staged, not rushed)

1. **alo Doc** (BlockNote) — the native document, on the existing Drive storage.
2. **alo Base** (relational) — the native data table, its own engine.
3. **Collabora compatibility types** (Writer/Calc/Impress via WOPI) — the
   fidelity layer.
4. **The AI layer** on top of all of them (ADR 0029), propose-then-approve.

Each ships fully and wire-verified before the next; each stage is honestly
labelled live-vs-in-progress.

## Rejected alternatives

- **One editor per format that does both.** A Word-compatible block editor is a
  contradiction; the result is a bad Word *and* a bad Notion. The split is the
  whole point.
- **Only the compatibility types (be a cheaper Office).** Concedes the
  differentiation; there is no reason to switch to us.
- **Only the native types (drop Office compatibility).** Loses every customer who
  must exchange real Office files — most of them.
- **Fork an open-source whole-app (NocoDB/Baserow/Grist-as-a-service).** Pulls a
  third language and an unowned product into our tree; violates the
  framework-not-whole-app and two-language rules. We build the native engine and
  embed frameworks.

## Consequences

- The document menu is honest about the trade: pick the great native tool, or the
  faithful-to-Office one, per document — no lossy compromise hidden inside one
  editor.
- Docs/Sheets/Slides all reuse Drive for storage, permission, versioning, and
  trash — built once (ADR 0027), not per editor.
- License provenance of each integrated framework (BlockNote MPL, AG Grid MIT /
  Univer Apache, KaTeX/Prism MIT, Collabora MPL) is recorded for the
  "confirm-before-monetizing" review; all are permissive/weak-copyleft under the
  AGPL+commercial model. No feature-gating — every feature is open.
