// A display-equation block (ADR 0015): the rendered equation with its auto-number,
// edited in the Figma equation modal. Numbering comes from the engine via the
// editor, so the "(n)" and any cross-reference to it stay correct.
import { useMemo, useState } from "react";

import { strings } from "../i18n";
import { cx } from "../ds";
import { renderMath } from "./katex";
import { EquationEditor } from "./EquationEditor";
import styles from "./EquationBlock.module.css";

interface EquationBlockProps {
  latex: string;
  numbered: boolean;
  /** The equation's current number, or undefined when not numbered. */
  number: string | undefined;
  onChange: (latex: string) => void;
  onToggleNumbered: (numbered: boolean) => void;
}

export function EquationBlock({
  latex,
  numbered,
  number,
  onChange,
  onToggleNumbered,
}: EquationBlockProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(latex);
  const rendered = useMemo(() => renderMath(latex, true), [latex]);

  return (
    <>
      <button
        type="button"
        className={styles.block}
        onClick={() => {
          setDraft(latex);
          setEditing(true);
        }}
      >
        <span className={cx(styles.math, rendered.error !== null && styles.error)}>
          {rendered.error !== null ? (
            latex.trim().length === 0 ? (
              strings.eqEmptyBlock
            ) : (
              latex
            )
          ) : (
            <span dangerouslySetInnerHTML={{ __html: rendered.html }} />
          )}
        </span>
        {numbered && number !== undefined && <span className={styles.number}>{`(${number})`}</span>}
      </button>

      {editing && (
        <EquationEditor
          value={draft}
          onChange={setDraft}
          display={true}
          numbered={numbered}
          onToggleNumbered={onToggleNumbered}
          onInsert={() => {
            onChange(draft);
            setEditing(false);
          }}
          onClose={() => setEditing(false)}
        />
      )}
    </>
  );
}
