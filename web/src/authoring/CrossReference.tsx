// Cross-reference chips and the insert-cross-reference picker (ADR 0015), styled
// to the Figma Docs screen. A chip stores the target item's id and renders its
// CURRENT number through the numbering engine — so it stays correct when items
// are reordered or inserted. The picker groups items into tabs and previews
// equations with rendered math.
import { useMemo, useState } from "react";
import { Link2 } from "lucide-react";

import { strings } from "../i18n";
import { cx } from "../ds";
import {
  type DocItem,
  type ItemKind,
  type NumberInfo,
  referenceText,
  resolveReference,
} from "./numbering";
import { renderMath } from "./katex";
import styles from "./CrossReference.module.css";

/** Localized reference-chip labels ("Eq.", "Table", …). */
export function refLabels(): Record<ItemKind, string> {
  return {
    section: strings.refSection,
    equation: strings.refEquation,
    table: strings.refTable,
    figure: strings.refFigure,
  };
}

/** An inline reference chip: resolves `targetId` to its current number, or shows
 * a broken state if the target was deleted. */
export function ReferenceChip({
  targetId,
  numbering,
}: {
  targetId: string;
  numbering: Map<string, NumberInfo>;
}) {
  const info = resolveReference(numbering, targetId);
  if (info === null) {
    return <span className={cx(styles.chip, styles.chipBroken)}>{strings.refBroken}</span>;
  }
  return (
    <span className={styles.chip}>
      <Link2 size={11} className={styles.chipIcon} />
      {referenceText(info, refLabels())}
    </span>
  );
}

/** A small rendered-math face for an equation preview in the picker. */
function MathPreview({ latex }: { latex: string }) {
  const r = useMemo(() => renderMath(latex, false), [latex]);
  if (r.error !== null) return <span className={styles.mathFallback}>{latex}</span>;
  return <span className={styles.math} dangerouslySetInnerHTML={{ __html: r.html }} />;
}

const TABS: ItemKind[] = ["equation", "section", "table", "figure"];

/** The insert-cross-reference picker: a tab per item kind, each listing that
 * kind's items with their number and (for equations) a rendered preview. */
export function CrossReferencePicker({
  items,
  numbering,
  onPick,
}: {
  items: DocItem[];
  numbering: Map<string, NumberInfo>;
  onPick: (id: string) => void;
}) {
  const [tab, setTab] = useState<ItemKind>("equation");

  const tabLabel: Record<ItemKind, string> = {
    equation: strings.refTabEquations,
    section: strings.refTabSections,
    table: strings.refTabTables,
    figure: strings.refTabFigures,
  };

  const listed = useMemo(() => items.filter((i) => i.kind === tab), [items, tab]);

  return (
    <div className={styles.picker} role="dialog" aria-label={strings.refInsertTitle}>
      <div className={styles.eyebrow}>{strings.refInsertTitle}</div>
      <div className={styles.tabs} role="tablist">
        {TABS.map((k) => (
          <button
            key={k}
            type="button"
            role="tab"
            aria-selected={tab === k}
            className={cx(styles.tab, tab === k && styles.tabOn)}
            onClick={() => setTab(k)}
          >
            {tabLabel[k]}
          </button>
        ))}
      </div>
      <div className={styles.list}>
        {listed.length === 0 ? (
          <div className={styles.empty}>{strings.refNoneOfKind}</div>
        ) : (
          listed.map((item) => {
            const info = numbering.get(item.id);
            return (
              <button
                key={item.id}
                type="button"
                className={styles.item}
                onClick={() => onPick(item.id)}
              >
                <span className={styles.itemNumber}>
                  {info !== undefined ? referenceText(info, refLabels()) : "—"}
                </span>
                {item.kind === "equation" && item.latex !== undefined ? (
                  <MathPreview latex={item.latex} />
                ) : (
                  <span className={styles.itemTitle}>{item.title}</span>
                )}
              </button>
            );
          })
        )}
      </div>
    </div>
  );
}
