# ADR 0031 — alo Doc: the block document model

Status: accepted. The first alo-native document type (ADR 0030), built on Drive
(ADR 0027). Defines how an alo Doc is stored, edited, versioned, and connected to
the rest of the workspace, and the propose-then-approve rule for its AI (ADR 0029).

## Context

alo Doc is the Notion-class document — where alo is *better* than Word/Google
Docs, not merely compatible. It should feel calm and block-based, and do things a
`.docx`-bound editor cannot: live data blocks, interactive blocks, technical
authoring (math/code/cross-references), and AI that proposes rather than acts.
The `.docx` compatibility case is a different type entirely (Collabora, ADR 0030)
— this ADR is only the native one.

## Decision

### It is a Drive node whose content is a block tree

An alo Doc is a `drive_node` with `kind = 'doc'` (ADR 0027). Its content is a
**BlockNote document** — a JSON tree of blocks — stored as a **blob**, exactly
like any other file's bytes. So an alo Doc inherits, for free:

- **Location + access** — it lives in personal My Files or a Space; whoever can
  see the location can open it (a Space doc is auto-shared with members).
- **Versioning** — every save appends a version (ADR 0027's `drive_node_versions`);
  history is kept and restorable, the block tree at each save preserved.
- **Trash, move, copy** — the same operations as any node; move re-scopes access.

Creating an alo Doc creates the node + an initial (empty-doc) blob as version 1.
Opening loads the current blob's JSON; saving stores a new blob and appends a
version. The store gains no doc-specific tables — a doc is content in a blob.

### The editor is BlockNote, embedded

We build on **BlockNote** (MPL-2.0, TypeScript) — an embeddable block editor
(blocks, slash-command insert, drag to reorder). It is a *framework we host*: alo
owns the surrounding chrome, the persistence, the block set we enable, and every
custom block. It renders **inside the workspace** (a Drive route/panel), never a
separate app or subdomain.

### The block set (staged)

- **v1 — the document.** Text blocks (paragraph, headings, lists, quote,
  divider, table, image, callout) with save-to-Drive + version history. A real,
  usable Notion-style doc.
- **Technical authoring.** Inline + display **math** (KaTeX) and
  syntax-highlighted **code** blocks (Prism) with a language picker, and
  **auto-numbered cross-references** (Eq./Table/Figure/Section that renumber on
  reorder). KaTeX + Prism are already vendored (the earlier authoring preview).
- **Interactive blocks.** Buttons, checklists, toggles — state stored in the
  block tree.
- **Linked-data blocks.** A table/view embedded from an alo Base (ADR 0032) or a
  sheet, rendered **live** so report numbers update when the underlying data
  changes. This is the cross-product move Notion/Docs cannot make, because alo
  has the workspace underneath — depends on alo Base existing first.

### Connected to the workspace

A doc lives in a Space and can link to real tasks, files, people, events, and
alo Base data in that Space (the source-link fabric, ADR 0029). These links are
plain data; they work with no AI involved.

### AI authoring is propose-then-approve, always (ADR 0029)

Every AI action proposes; the user approves. Nothing is applied silently:
clean-paste (offer to strip foreign formatting, keep-original escape hatch),
ask-AI-from-your-docs (answers from the user's real files, source-cited), inline
command bar (rewrite/shorten/fix on a selection) shown as an **accept/reject
diff**, and agent mode as a **visible plan**. The code rule: an AI mutation path
writes a *proposal* the user promotes — a silent AI write is a bug, as in tasks
(ADR 0023).

### Real-time co-editing is a later slice (flagged)

v1 is single-writer with explicit save + version history — honest and useful.
Live multi-cursor co-editing needs a CRDT/sync backend (e.g. Yjs) and is a
separate, later slice; it is **not** claimed until built. Until then, a doc open
in two places is last-save-wins with full version history to recover — stated
plainly in the UI, not hidden.

## Rejected alternatives

- **Store the doc as HTML/Markdown.** Lossy for the block model (interactive and
  linked-data blocks have no HTML), and a poor merge substrate for future
  co-editing. JSON block tree is the native, lossless form.
- **A dedicated `documents` table/service.** A doc is content; Drive already
  stores content (blobs), scopes it (location), and versions it. A parallel store
  duplicates all three and breaks "a doc is a file you can move into a Space".
- **Fork a whole Notion-clone app.** Pulls an unowned product (and often a third
  language) into the tree. BlockNote is a framework we embed and own around.
- **Claim co-editing in v1 without the sync backend.** Dishonest; last-save-wins
  dressed up as collaboration is exactly the bug the "done means it works" law
  forbids.

## Consequences

- alo Doc reuses Drive entirely for storage/permission/versioning — the editor
  is the only genuinely new surface, and it is a framework we embed.
- The differentiators (linked-data, interactive, technical, AI) are staged on top
  of a real v1 document, each shippable and honestly labelled.
- Linked-data blocks make alo Base a dependency for that feature — sequencing
  alo Doc first (v1) then alo Base then linked-data blocks.
- BlockNote's MPL license is recorded for the confirm-before-monetizing review.
