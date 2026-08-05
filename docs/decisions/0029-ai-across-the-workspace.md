# ADR 0029 — AI across the workspace, and propose-then-approve document AI

Status: accepted (direction); implementation staged behind the Drive/Docs
surface and the AI layer. Records two commitments that shape every AI feature in
the file/document workspace, so later code is measured against them.

## Context

The differentiation is not "a cheaper European Office". It is that **one AI sees
the whole workspace** — files, mail, and tasks together — and that the AI is a
trustworthy collaborator, never a silent editor. Both are only possible because
of decisions already made: the shared data layer (one brain, ADR 0026), the
source-link pattern (ADR 0021/0027), and propose-then-approve (ADR 0023). This
ADR ties them into standing rules for the document workspace.

## Decision

### 1. AI reads across modules; one query, cited answers

Search and AI operate over **files (name + extracted content), mail, and
tasks** as one corpus, scoped to the caller's tenant and to exactly what they
may already see (their personal items + the Spaces they belong to + their
mailbox). "Find the Acme proposal Marie sent" fans out across all three and
ranks results.

- **Access is never widened by AI.** The retrieval layer applies the same
  membership/location predicates as a direct read; the AI can surface only what
  the user could already open. There is no privileged index.
- **Every AI answer is source-cited** — which file, which message, which task
  it came from — so the user can verify, and jump to the source (the existing
  jump-back links).
- Search ships in stages: **name/metadata first** (immediately useful), then
  **content** (extraction + full-text), then **semantic/AI ranking**. Each
  stage is honest about its reach; none is faked.

### 2. Source links make the workspace one fabric

A file can carry a `source_kind`/`source_id` back to the email, task, or event
it came from (ADR 0027). Saving a mail attachment to Drive keeps the link to the
message; a doc can be tied to a meeting; a file can be attached to a task or
event. These links are the connective tissue the cross-module AI reasons over,
and they are plain data, not an AI feature — they work without any model.

### 3. Document AI is propose-then-approve, always (alo's signature)

Every AI action in Docs/Sheets/Slides **proposes**; the user **approves**.
Nothing is ever applied silently. This mirrors the task module's proposed-then-
accepted state (ADR 0023) and is non-negotiable across the surface:

- **Docs** — clean-paste (offer to strip foreign formatting, keep-original
  escape hatch), ask-AI-from-your-docs (cited), inline agentic edits shown as
  an **accept/reject diff**, linked-data blocks (a doc table synced from a
  sheet, shown as a proposed update).
- **Sheets** — natural-language formulas, explain-and-fix errors, ask-your-
  data, paste-guard — each surfaced as a proposal the user commits.
- **Slides** — deck-from-context, brand-apply, design-cleanup, data-to-slide —
  generated as a draft the user accepts, never an overwrite.

The rule the code enforces: an AI mutation path always writes a *proposal*
first; only a user action promotes it to the document. A silent AI write is a
bug, exactly as it is in tasks.

## Rejected alternatives

- **A separate AI index with its own permissions.** A second source of truth
  for "who can see what" — the ADR 0026 anti-pattern, and a leak waiting to
  happen. Retrieval reuses the live predicates.
- **Auto-applying AI edits with an undo.** "Undo" is not consent; the product's
  trust promise is that the AI proposes. Undo-only is how the incumbents do it
  and precisely the behaviour users distrust.
- **Per-module AIs (a mail AI, a docs AI).** Defeats the one-brain premise; the
  cross-module query is the differentiator.

## Consequences

- The document workspace can advertise, truthfully, that the AI never changes a
  document without approval and never sees more than the user can.
- Retrieval and doc-AI both depend on the shared data layer — vindicating the
  "one brain" decision (workspace codebase over a standalone Drive service).
- Implementation is staged (name-search → content-search → Collabora doc AI →
  cross-module semantic search); each stage is shippable and honestly labelled,
  and each is held to the two commitments above.
