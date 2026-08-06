# ADR 0033 — Native-only editors: Collabora removed, alo Slides built in-house

Status: accepted. **Supersedes the Collabora dependency in [ADR 0010]
(0010-editors-collabora-shell.md) and [ADR 0030]
(0030-two-file-types-per-format.md).** The native-type decisions in 0030/0031/
0032 stand; the *compatibility-via-Collabora* half of 0030 is withdrawn.

## Context

ADR 0010 made the Office editors a alo-branded **shell over Collabora**, and
0030 kept Collabora as the compatibility engine for real `.docx`/`.xlsx`/`.pptx`.
Two facts, established on the wire, make that untenable:

1. **Collabora cannot be de-branded on a build we may run.** We run the free
   `collabora/code` (CODE) image, which shows Collabora's identity inside the
   editing canvas. Full white-labelling is a feature of Collabora's *paid* tier.
   Removing the branding ourselves would mean **patching the engine**, which
   doctrine forbids ("engines are configured, never patched"). So the shell of
   0010 can wrap the frame, but the canvas the user actually types in stays
   visibly Collabora. For a sovereignty product sold on "everything is ours,"
   a third party's brand in the core editing surface is a defect, not a detail.

2. **No free engine replaces it like-for-like.** Univer's free (Apache-2.0)
   build edits *its own* document model — it cannot open real Office files
   (that needs Univer's paid server), and it has **no slides product** at all in
   the free tier. LibreOffice-family engines are Collabora-equivalent.

The product owner's decision, recorded here: **remove Collabora entirely; make
the whole document surface native and fully owned; where no free engine exists,
build our own.** This is the "new fact" 0030 asked for before revisiting its
"no native slide canvas" stance — the fact is that *keeping* Collabora is now
disqualifying, so the previously-declined slide editor becomes required.

## Decision

**The document surface is native-only. Collabora, WOPI, and the Office shell are
removed from the product and the deployment.**

| Format | The editor (native, alo-owned) | A real Office file (`.docx`/`.xlsx`/`.pptx`) |
|---|---|---|
| Docs   | **alo Doc** — BlockNote (ADR 0031)      | **best-effort import** into an alo Doc |
| Sheets | **alo Sheet** — Univer                  | **best-effort import** into an alo Sheet |
| Slides | **alo Slides** — *our own editor (new)* | **best-effort import** into alo Slides |

- **Best-effort import, not round-trip fidelity.** Opening a real Office file
  converts it into the matching native type: structure and simple formatting
  (headings, paragraphs, emphasis, lists, tables; cell values, basic styles;
  slide text and images) carry over; complex layout, macros, and exact
  rendering do not. We **explicitly drop the fidelity promise** of 0030 — alo is
  the native, sovereign alternative to Office, not a faithful Office editor.
  The import is one-way; the result is a alo document from then on. The original
  file stays in Drive, downloadable, unmodified.
- **alo Slides is built in-house.** A native slide canvas (slides, text boxes,
  shapes, images, per-slide layout). It is the largest build and the last of the
  native homes; Collabora is not removed until it exists, so no format is ever
  stranded. Slide *fidelity* is not a goal; a clean native deck editor is.
- **Two languages, framework-not-app still hold.** Importers use permissively
  licensed JS/Rust libraries (e.g. SheetJS/`xlsx` for spreadsheets, a
  WordprocessingML→HTML step for documents, our existing Rust OOXML parsing in
  `alo-store` for `.pptx` text/media) — libraries we embed, never a whole app we
  fork, and never a third language.

### Rejected alternatives

- **Keep Collabora as a hidden "compatibility mode."** Rejected by the owner:
  any third-party brand in the editing surface is unacceptable, and it keeps a
  heavy engine + WOPI backend alive for a shrinking case.
- **Pay for Collabora's white-label / server tier.** Rejected: recurring cost
  and a licensed dependency at the core of a sovereignty product.
- **Drop slides entirely (leave `.pptx` view-only forever).** Rejected: the
  owner chose to build rather than omit; a native deck editor is in scope.

## Consequences

- **Lost:** faithful round-trip of real Office files to desktop Office. This is a
  real cost for a business mail product (attachments arrive as Office files) and
  must be stated plainly to users at the import boundary ("Imported as an alo
  Doc — formatting may differ; the original stays in Drive").
- **Gained:** one consistent, brand-free, fully-owned editing surface; no paid
  engine, no engine patching, no WOPI attack surface, a lighter deployment
  (the `collabora/code` container and its Caddy/`net.frame_ancestors` config go
  away).
- **Build:** three import paths + one new editor (alo Slides). Removal touches
  `deploy/` (compose, Caddy, `.env`), the JMAP WOPI backend (`wopi.rs` + routes),
  and the web Office shell (`OfficeEditor`, `OFFICE_HOST`, the dev proxy's
  Collabora paths).
- **Rollout is staged so nothing breaks mid-flight:** (1) `.xlsx`→alo Sheet,
  (2) `.docx`→alo Doc, (3) alo Slides + `.pptx`, (4) remove Collabora. Collabora
  keeps serving real Office files until stage 4; each stage ships working.
- **Contracts:** the WOPI HTTP surface and `hosting/discovery` are removed, not
  versioned — they are internal engine glue, never a public alo contract, so
  their removal breaks no external caller. The Office MIME node kinds in Drive
  remain valid files; only how they *open* changes.

## Follow-ups

- Supersede the affected lines in ADR 0010 and 0030 with a pointer here.
- ROADMAP: replace "Collabora embedded, alo-themed" and the Office-fidelity CI
  line with the four native stages above.
- A new ADR for the alo Slides document model (parallel to 0031/0032) when
  stage 3 begins.
