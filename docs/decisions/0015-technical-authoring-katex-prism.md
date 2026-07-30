# ADR 0015 — Technical authoring: KaTeX + Prism, browser-local, Ficina owns numbering

**Status:** accepted · 2026-07

**Decision:** Ficina Docs gains a **technical-authoring** capability — inline and
numbered display **math** (LaTeX input), syntax-highlighted **code blocks**, and
**auto-numbered cross-references** (equations, tables, figures, sections). Math
is rendered by **KaTeX** (MIT) and code by **Prism** (MIT), both running
**entirely in the browser** with no server round-trip and no external call. The
**auto-numbering and cross-reference layer is Ficina's own code**, not a library
feature. This is an ADR 0010 *shell* capability: Ficina owns the authoring
surface; when the Collabora Docs integration lands, the same components dock into
that shell. The module ships **standalone first** (it renders in the web app on
its own) so it is verifiable before Collabora exists.

**Why browser-local (the sovereignty point).** A sovereignty product must not
send a customer's draft equations or source code to a third-party service to
render them. KaTeX and Prism both render on the client from the raw input — the
LaTeX and the code never leave the browser. This is the same doctrine as ADR
0011 keeping inference in-region, applied to rendering: **the content is the
customer's, and it stays local.** It is also faster — KaTeX renders
synchronously, with no network latency — which is why it is the industry default
for local math.

**Why KaTeX for math (with MathJax as a documented fallback).** KaTeX renders a
large, well-defined subset of LaTeX math synchronously and self-contained (its
fonts ship with the library; no CDN). It covers inline math and numbered display
equations — our entire requirement — and is the fastest local renderer.
**MathJax (Apache-2.0)** is recorded here as the escape hatch **if** a customer
ever needs notation outside KaTeX's supported set: MathJax consumes the **same
LaTeX input**, so switching the renderer is a component-internal change with **no
change to stored content and no lock-in**. We do not ship MathJax now — adding a
second, heavier renderer for notation no one has asked for would be scope we
cannot justify; the seam (a single `renderMath(latex, …)` function) is where it
would slot in.

**Why Prism for code, language chosen explicitly.** Prism is a small, MIT
highlighter with per-language grammars loaded on demand. The language is **set
explicitly by the author via the language picker**, never guessed — auto-detection
is unreliable and would mis-highlight a finance spec's SQL as something else.
Explicit language is also what round-trips cleanly into a stored document model
(the language is a property of the block, not a heuristic re-run on every render).

**Why Ficina owns numbering and cross-references.** This is the part no library
gives us and the part that must be *correct*: equations, tables, figures, and
sections carry **stable identities** and are assigned **display numbers by their
order in the document**; a cross-reference ("Eq. 3", "Table 1", "Section 2.3") is
stored as a **reference to the identity, resolved to the current number at render
time**. Inserting or reordering items renumbers everything and every reference
updates automatically — because references point at identities, never at a baked-in
number. Section numbers are hierarchical (`2.3`); equation/table/figure numbers are
per-type sequences. KaTeX deliberately does **not** do this (it renders one formula;
it knows nothing about the document), so building it on Ficina's side is required,
not a preference. It lives as a pure, tested module (`web/src/authoring/numbering.ts`)
with no rendering dependency, so its correctness is unit-tested independently of KaTeX
or Prism.

**Licensing.** KaTeX and Prism are **MIT**; safe to bundle and ship in the
commercial product. LaTeX *the language* is free/open (LPPL) and we use only the
KaTeX renderer, **not** a TeX distribution — there is no TeX runtime, no GPL
surface, and no per-seat licensing anywhere in this path. MathJax, if ever
adopted, is Apache-2.0 — also commercial-safe. This keeps us inside the
"two languages only" rule: these are TypeScript-above-the-waterline libraries;
no new language enters the repo.

**Contracts / compatibility.** Additive and frontend-only in this slice: two new
runtime dependencies (`katex`, `prismjs`), a new `web/src/authoring/` module, and
a Ficina Docs surface that renders it. No backend, JMAP method, HTTP route, or
schema changes. When technical-authoring content is later **persisted**, the
stored shape (a block model: `{type, latex|code|…, id}` for math/code, plus the
identity/number registry for cross-references) is the contract to version then;
this ADR fixes the rendering and numbering decisions it will build on.

**Rejected — render math server-side (e.g. a TeX service).** It would send
customer content off the client to render, contradicting the sovereignty
promise, add a server dependency and latency to every keystroke of a live
preview, and buy nothing KaTeX does not already give us locally.

**Rejected — auto-detect the code language.** Guessing mis-highlights real
documents (a SQL block read as shell, a `.tsx` read as XML) and makes the
rendered output depend on a heuristic rather than the author's stated intent.
The picker is explicit; the language is a stored property of the block.

**Rejected — let a library own numbering, or bake numbers into references.**
No renderer owns cross-document numbering, and a reference that stores "3"
instead of "the third equation's identity" silently goes wrong the moment
anything is inserted or reordered — the exact bug this feature exists to
prevent. Numbering is Ficina's, keyed on identity, resolved at render.
