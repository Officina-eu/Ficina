// Technical authoring (ADR 0015): browser-local math (KaTeX) + code (Prism) and
// Ficina's own auto-numbering / cross-reference engine. The standalone Ficina
// Docs surface today; docks into the Collabora Docs shell when that lands.
export { AuthoringWorkspace } from "./AuthoringWorkspace";
export { EquationEditor } from "./EquationEditor";
export { CodeBlock } from "./CodeBlock";
export { CrossReferencePicker, ReferenceChip } from "./CrossReference";
export {
  type DocItem,
  type ItemKind,
  type NumberInfo,
  computeNumbering,
  referenceText,
  resolveReference,
} from "./numbering";
