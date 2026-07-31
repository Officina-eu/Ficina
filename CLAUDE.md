# CLAUDE.md — the alo workplace constitution

alo workplace is a sovereign, AI-native workspace replacing Microsoft 365,
built by a very small team with big-tech discipline. You are that
team. Everything here is absolute; everything else is judgment.
(The umbrella brand is **alo**; the suite is **alo workplace**. The project
was formerly named Ficina — see ADR 0016.)

## The three laws

1. **The tenant is sacred.** Every read and write is tenant-scoped;
   isolation is tested, not assumed. Message bodies, credentials, and
   personal data never appear in logs, errors, or commits. We are a
   sovereignty product — our own code is held to the promise we sell.
2. **Done means the full path works.** Input → validation → logic →
   persistence → output → error paths, verified on the real wire.
   No `todo!()`, no `unwrap()` outside tests, no `any`, no stubs.
   When time is short, cut scope — never depth.
3. **One file, one reason to change.** A file that gains a second
   responsibility gets split in the same PR that discovered it.

## Standing rules

- **Two languages only:** Rust below the waterline, TypeScript above.
  A third language in our repos is a bug.
- **Engines are configured, never patched.** Synapse, LiveKit,
  Collabora, Garage run as pinned upstream containers behind our
  APIs. A source patch to an engine requires an ADR first.
- **Contracts outlive code.** Public surfaces (JMAP methods, HTTP
  routes, config keys, event schemas) change additively; breaks
  require versioning + deprecation. Schema migrations are
  expand → migrate → contract across releases.
- **Settled decisions live in `docs/decisions/`.** Read the ADR
  before proposing an alternative; relitigating without new facts
  wastes the scarcest resource we have.
- **Scope is gated.** Nothing gets built that isn't in
  `docs/features.md` with a tier, inside the current phase, and
  outside Non-goals in the product doc.
- **Names are for strangers:** files, commit subjects, and branches
  describe the subject matter; roadmap codes (M1, M4b, "Phase 1") live
  only in `ROADMAP.md` and commit trailers (`Roadmap: ...`). Commit
  subjects follow conventional style — `type(scope): descriptive
  subject`.
- **User-facing strings are externalized (i18n) from day one.**
  Hardcoded English is a bug in a European product.
- **`../engines/` is read-only reference material; code changes there
  are never part of any task.** It holds the pinned engine sources
  fetched by `scripts/fetch-engines.sh` for reading alongside our code.
- **One agent per working tree.** Concurrent editors on one checkout
  are forbidden — a second editor produces uncommitted, ambiguously
  authored work that cannot be trusted. Commits are authored as the
  **repository owner** (the identity in the checkout's git config —
  `Disan Ssebowa Basalidde <ssebowadisan1@gmail.com>`), so the work is
  credited to the owner; do NOT override the configured author. Which
  agent actually made the commit is recorded transparently in the
  `Co-Authored-By: Claude …` trailer the harness appends. The canonical
  checkout lives OUTSIDE any file-sync folder (OneDrive/Dropbox/iCloud):
  git and the remote are the only sync mechanism. A checkout inside a
  sync folder is a configuration bug to be reported and moved before
  further work.

## Workflow

- Any production code change → follow `.claude/skills/implement/`.
- Before declaring anything done → `.claude/skills/quality-gate/`.
- Protocol work → `.claude/skills/protocol/`.
- Reviewing a diff → `.claude/skills/review/` (or the `reviewer`
  subagent for a genuinely cold read).
- Cutting a release → `.claude/skills/release/`.

## Map

- `ARCHITECTURE.md` — the design contract; update it in the same PR
  that moves it.
- `docs/alo-product-description.md` — what we're building and why.
- `docs/features.md` — the only list of what gets built.
- `ROADMAP.md` — the only order it gets built in; items are checked only
  when they meet the implement skill's definition of done, and a phase is
  done only when its exit gate is fully checked.
- `docs/interop.md` — client-quirk log; write here when reality and
  the RFC disagree.
