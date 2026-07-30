// Rendering for paragraph prose (ADR 0015). A paragraph's text may embed inline
// math as `$…$` and cross-references as `{{ref:ID}}`; `renderProse` turns that
// into React nodes — plain text, inline KaTeX, and live reference chips that
// resolve to the current number through the numbering engine.
import { Fragment, type ReactNode, useMemo } from "react";

import type { NumberInfo } from "./numbering";
import { renderMath } from "./katex";
import { ReferenceChip } from "./CrossReference";
import styles from "./prose.module.css";

/** A single inline math fragment. */
export function InlineMath({ latex }: { latex: string }) {
  const r = useMemo(() => renderMath(latex, false), [latex]);
  if (r.error !== null) return <span className={styles.mathError}>${latex}$</span>;
  return <span dangerouslySetInnerHTML={{ __html: r.html }} />;
}

const TOKEN = /\$([^$]+)\$|\{\{ref:([^}]+)\}\}/g;

/** Render paragraph text with inline math and cross-reference chips resolved. */
export function renderProse(text: string, numbering: Map<string, NumberInfo>): ReactNode {
  const nodes: ReactNode[] = [];
  let last = 0;
  let key = 0;
  let m: RegExpExecArray | null;
  TOKEN.lastIndex = 0;
  while ((m = TOKEN.exec(text)) !== null) {
    if (m.index > last) nodes.push(<Fragment key={key++}>{text.slice(last, m.index)}</Fragment>);
    if (m[1] !== undefined) {
      nodes.push(<InlineMath key={key++} latex={m[1]} />);
    } else if (m[2] !== undefined) {
      nodes.push(<ReferenceChip key={key++} targetId={m[2]} numbering={numbering} />);
    }
    last = TOKEN.lastIndex;
  }
  if (last < text.length) nodes.push(<Fragment key={key++}>{text.slice(last)}</Fragment>);
  return nodes;
}
