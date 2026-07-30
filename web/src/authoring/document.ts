// The Ficina Docs document model (ADR 0015): a document is an ordered list of
// blocks. Equations, tables, and headings (sections) are referenceable — the
// numbering engine assigns their numbers from block order, and cross-references
// stored in paragraph text resolve to the current number. This model is what the
// /docs API persists as JSON.
import type { DocItem } from "./numbering";

export type Block =
  | { type: "heading"; id: string; level: 1 | 2; text: string }
  /** Prose. `text` may embed inline math as `$…$` and cross-references as `{{ref:ID}}`. */
  | { type: "paragraph"; id: string; text: string }
  | { type: "equation"; id: string; latex: string; numbered: boolean }
  | { type: "code"; id: string; code: string; language: string }
  /** A table; `rows[0]` is the header row. */
  | { type: "table"; id: string; rows: string[][] };

export type BlockType = Block["type"];

export interface DocumentSummary {
  id: string;
  title: string;
  updatedAt: string;
}

export interface DocumentDoc extends DocumentSummary {
  blocks: Block[];
}

/** A fresh, collision-resistant block id. */
export function newId(): string {
  return crypto.randomUUID();
}

/** Build a default block of the given kind. */
export function blankBlock(type: BlockType): Block {
  const id = newId();
  switch (type) {
    case "heading":
      return { type, id, level: 2, text: "" };
    case "paragraph":
      return { type, id, text: "" };
    case "equation":
      return { type, id, latex: "", numbered: true };
    case "code":
      return { type, id, code: "", language: "typescript" };
    case "table":
      return {
        type,
        id,
        rows: [
          ["Column A", "Column B"],
          ["", ""],
        ],
      };
  }
}

/** The starter blocks for a new document (the title is document metadata, so
 * the body just opens with an empty paragraph). */
export function starterBlocks(): Block[] {
  return [{ type: "paragraph", id: newId(), text: "" }];
}

/** The title cell of a table (first header cell), for the cross-reference picker. */
function tableTitle(block: Extract<Block, { type: "table" }>): string {
  const first = block.rows[0]?.[0]?.trim();
  return first !== undefined && first.length > 0 ? first : "Table";
}

/**
 * Derive the numbered items (in document order) that the numbering engine works
 * on: headings become sections, equations equations, tables tables. Paragraphs
 * and code are not numbered.
 */
export function blocksToItems(blocks: Block[]): DocItem[] {
  const items: DocItem[] = [];
  for (const block of blocks) {
    if (block.type === "heading") {
      items.push({ id: block.id, kind: "section", level: block.level, title: block.text });
    } else if (block.type === "equation") {
      items.push({ id: block.id, kind: "equation", title: "Equation", latex: block.latex });
    } else if (block.type === "table") {
      items.push({ id: block.id, kind: "table", title: tableTitle(block) });
    }
  }
  return items;
}
