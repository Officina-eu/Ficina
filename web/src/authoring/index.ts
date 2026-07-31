// Technical authoring (ADR 0015): browser-local math (KaTeX) + code (Prism), a
// tenant/owner-scoped document store, and alo's own auto-numbering /
// cross-reference engine. The standalone alo Docs surface today; docks into
// the Collabora Docs shell when that lands.
export { DocsModule } from "./DocsModule";
export { DocumentEditor } from "./DocumentEditor";
export { EquationEditor } from "./EquationEditor";
export { CodeBlock } from "./CodeBlock";
export { CrossReferencePicker, ReferenceChip } from "./CrossReference";
export { AuthoringInsertModal } from "./AuthoringInsertModal";
export { codeEmailHtml, equationEmailHtml } from "./emailBlocks";
export type { Block, DocumentDoc, DocumentSummary } from "./document";
export {
  type DocItem,
  type ItemKind,
  type NumberInfo,
  computeNumbering,
  referenceText,
  resolveReference,
} from "./numbering";
