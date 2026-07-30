// The cross-reference and auto-numbering engine — Ficina's own logic, not a
// library feature (ADR 0015). Equations, tables, figures, and sections are
// numbered by their ORDER in the document; a cross-reference stores the target
// item's stable `id` and is resolved to that item's CURRENT number at render
// time. Because references point at identities and numbers are recomputed from
// order, inserting or reordering items renumbers everything and every reference
// updates automatically. This module is pure (no rendering, no DOM) so its
// correctness is unit-tested independently of KaTeX and Prism.

export type ItemKind = "section" | "equation" | "table" | "figure";

/** One numbered item in a document, in document order. */
export interface DocItem {
  /** Stable identity — a reference targets this, never the computed number. */
  id: string;
  kind: ItemKind;
  /** Sections only: heading depth (1 = top level). Ignored for other kinds. */
  level?: number;
  /** Title/caption, for display in the cross-reference picker. */
  title?: string;
  /** Equations only: LaTeX, so the picker can render a preview of the target. */
  latex?: string;
}

export interface NumberInfo {
  kind: ItemKind;
  /** The computed display number: "2.3" for sections, "3" for equation/table/figure. */
  display: string;
}

/** English reference-chip labels; the UI passes localized labels for display. */
export const DEFAULT_LABELS: Record<ItemKind, string> = {
  section: "Section",
  equation: "Eq.",
  table: "Table",
  figure: "Figure",
};

/** Deepest section nesting we number; beyond this, extra depth is clamped. */
const SECTION_MAX_DEPTH = 6;

/**
 * Assign a display number to every item, by document order.
 *
 * - Sections are hierarchical: a level-1 section is "1", a level-2 under it
 *   "1.1", and so on; entering a shallower level resets the deeper counters.
 * - Equations, tables, and figures each get their own running sequence
 *   ("1", "2", "3", …) across the whole document.
 */
export function computeNumbering(items: readonly DocItem[]): Map<string, NumberInfo> {
  const out = new Map<string, NumberInfo>();
  const sectionCounters: number[] = []; // index i holds the counter for level i+1
  const seq: Record<Exclude<ItemKind, "section">, number> = {
    equation: 0,
    table: 0,
    figure: 0,
  };

  for (const item of items) {
    if (item.kind === "section") {
      const level = Math.min(Math.max(item.level ?? 1, 1), SECTION_MAX_DEPTH);
      // Trim any deeper counters (they reset when we come back up a level), and
      // make sure every counter up to this level exists.
      sectionCounters.length = level;
      for (let i = 0; i < level; i++) {
        if (sectionCounters[i] === undefined) sectionCounters[i] = 0;
      }
      sectionCounters[level - 1] = (sectionCounters[level - 1] ?? 0) + 1;
      out.set(item.id, { kind: "section", display: sectionCounters.join(".") });
    } else {
      seq[item.kind] += 1;
      out.set(item.id, { kind: item.kind, display: String(seq[item.kind]) });
    }
  }

  return out;
}

/** The text of a reference chip, e.g. "Eq. 3" / "Table 1" / "Section 2.3". */
export function referenceText(
  info: NumberInfo,
  labels: Record<ItemKind, string> = DEFAULT_LABELS,
): string {
  return `${labels[info.kind]} ${info.display}`;
}

/**
 * Resolve a cross-reference to the target item's current number, or `null` when
 * the target no longer exists (a dangling reference — the UI renders it as a
 * broken chip rather than a wrong number).
 */
export function resolveReference(
  numbering: Map<string, NumberInfo>,
  targetId: string,
): NumberInfo | null {
  return numbering.get(targetId) ?? null;
}
