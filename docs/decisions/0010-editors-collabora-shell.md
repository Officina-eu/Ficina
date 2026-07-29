# ADR 0010 — Office editors are a Ficina shell over Collabora, not a from-scratch editor

**Status:** accepted · 2026-07

**Decision:** Ficina Docs, Sheets, and Slides are a **Ficina-branded,
AI-native shell over the integrated Collabora Online engine** (reached via
**WOPI**), not a from-scratch editing engine. This is ADR 0003 (engines are
integrated, not built) applied to the office editors, made explicit because
it fixes a boundary a major module depends on:

- **Collabora provides:** the editing engine — the .docx/.xlsx/.pptx (and
  .odt/.ods/.odp) rendering and manipulation, the formula and layout engines,
  real-time co-editing, and desktop-Office **format fidelity** (the whole
  ballgame — a mangled offer letter loses the customer).
- **Ficina owns:** the branded shell (the frame, chrome, navigation, and the
  single-version web-first UX), the **AI layer** (the editor-native
  inventions — see `docs/features.md` "Ficina Docs" / "Ficina Sheets"), the
  WOPI host (served by Drive), version/lineage surfaces, and the
  workspace-context grounding that only a suite owner can provide.

**Why:** Building a from-scratch office editor with genuine Microsoft-format
fidelity is a decade-scale effort (LibreOffice/Collabora represents ~25 years
of work); attempting it would consume the entire company and still lose on
fidelity, which is the one thing that must be perfect. Ficina's
differentiation is **not** the editing engine — it is being **AI-native,
whole-suite, and sovereign**. The four inventions per editor (clean paste,
ask-AI-from-your-docs, semantic-conflict flag, draft-from-workspace-context;
explain-and-fix errors, natural-language formulas, formula paste-guard,
ask-your-data) live in the layer Ficina owns, and are only possible because
Ficina owns Mail + Meet + Drive + Docs in one place. Owning the shell and the
AI layer, and integrating the engine, puts every hour of our effort on the
differentiation and none on re-deriving a solved problem.

**The AI layer's trust model is part of this decision:** the AI **proposes
and diffs; the user accepts**. AI never silently overwrites a document or a
formula — inline commands produce a reviewable diff, and agent-mode
multi-step tasks run as a visible, stoppable plan. This is the trust posture
a sovereignty product requires, and it is Ficina's to own regardless of the
engine underneath.

**Rejected — build a from-scratch editor** (or fork and diverge from
LibreOffice core). Rejected on the fidelity and cost grounds above, and
because a divergent fork forfeits upstream security and format-support
updates — the opposite of the pinned-engine discipline (ADR 0003). A source
patch to Collabora requires its own ADR.

**Rejected — a thin iframe with no Ficina layer** (ship Collabora as-is).
Rejected because it forfeits the entire differentiation: no AI inventions, no
cross-suite grounding, no single-version web-first UX, no Ficina consent/
audit/version story — it would be "Collabora with our logo," which is not a
product. The shell and AI layer are the product.

**Consequences:** The editor surfaces (the shell UX, the AI invention
behaviours, the WOPI contract) are **Ficina-owned public surfaces** — they
change additively (CLAUDE.md "contracts outlive code"), and their UX source
of truth is the Figma design (pages "10 · Docs" and "11 · Sheets"). The base
editing capabilities and the format-fidelity CI corpus are in ROADMAP Phase 2
("Drive & Docs"); the AI-native inventions depend on both that Collabora
integration and the Phase 3 AI layer, so they are scheduled in **Phase 3**,
never Phase 1. **VBA macros remain an honest non-goal** (they do not run in
the engine) — the migration playbook's answer stands (`docs/features.md`,
product doc §6).
