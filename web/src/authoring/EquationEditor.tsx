// The equation editor (ADR 0015), styled to the Figma Docs "Equation" modal: a
// Σ-marked panel with a LaTeX/Visual view toggle, a LaTeX source input, a live
// centered KaTeX preview, a common-symbol quick bar, and an Insert button.
// Rendering is browser-local; invalid LaTeX shows inline and never breaks the page.
import { useMemo, useRef, useState } from "react";
import { Sigma, X } from "lucide-react";

import { strings } from "../i18n";
import { cx } from "../ds";
import { renderMath } from "./katex";
import styles from "./EquationEditor.module.css";

interface Symbol {
  tip: string;
  face: string;
  insert: string;
  /** Caret offset from the end of `insert` (negative = inside braces). */
  caret?: number;
}

// The symbol set shown in the Figma modal.
const SYMBOLS: Symbol[] = [
  { tip: "Sum", face: "\\sum", insert: "\\sum_{}^{}", caret: -3 },
  { tip: "Integral", face: "\\int", insert: "\\int_{}^{}", caret: -3 },
  { tip: "Square root", face: "\\sqrt{x}", insert: "\\sqrt{}", caret: -1 },
  { tip: "Pi", face: "\\pi", insert: "\\pi " },
  { tip: "Infinity", face: "\\infty", insert: "\\infty " },
  { tip: "Less or equal", face: "\\le", insert: "\\le " },
  { tip: "Greater or equal", face: "\\ge", insert: "\\ge " },
  { tip: "Alpha", face: "\\alpha", insert: "\\alpha " },
  { tip: "Beta", face: "\\beta", insert: "\\beta " },
];

function SymbolButton({ symbol, onInsert }: { symbol: Symbol; onInsert: (s: Symbol) => void }) {
  const face = useMemo(() => renderMath(symbol.face, false), [symbol.face]);
  return (
    <button
      type="button"
      className={styles.symbol}
      title={symbol.tip}
      aria-label={symbol.tip}
      onClick={() => onInsert(symbol)}
      dangerouslySetInnerHTML={{ __html: face.html }}
    />
  );
}

interface EquationEditorProps {
  value: string;
  onChange: (latex: string) => void;
  /** Render the preview as a display (block) equation vs inline math. */
  display: boolean;
  /** Confirm and place the equation. */
  onInsert: () => void;
  onClose: () => void;
  /** Optional "numbered display equation" toggle. */
  numbered?: boolean;
  onToggleNumbered?: (numbered: boolean) => void;
}

export function EquationEditor({
  value,
  onChange,
  display,
  onInsert,
  onClose,
  numbered,
  onToggleNumbered,
}: EquationEditorProps) {
  const [view, setView] = useState<"latex" | "visual">("latex");
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const rendered = useMemo(() => renderMath(value, display), [value, display]);

  function insert(symbol: Symbol) {
    const el = inputRef.current;
    const start = el?.selectionStart ?? value.length;
    const end = el?.selectionEnd ?? value.length;
    const next = value.slice(0, start) + symbol.insert + value.slice(end);
    onChange(next);
    const caret = start + symbol.insert.length + (symbol.caret ?? 0);
    requestAnimationFrame(() => {
      el?.focus();
      el?.setSelectionRange(caret, caret);
    });
  }

  return (
    <div className={styles.overlay} onMouseDown={onClose}>
      <div
        className={styles.modal}
        role="dialog"
        aria-label={strings.eqTitle}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className={styles.head}>
          <Sigma size={18} className={styles.headIcon} />
          <span className={styles.headTitle}>{strings.eqTitle}</span>
          <div className={styles.spacer} />
          <div className={styles.segment} role="group" aria-label={strings.eqViewLabel}>
            <button
              type="button"
              className={cx(styles.segBtn, view === "latex" && styles.segOn)}
              onClick={() => setView("latex")}
            >
              {strings.eqViewLatex}
            </button>
            <button
              type="button"
              className={cx(styles.segBtn, view === "visual" && styles.segOn)}
              onClick={() => setView("visual")}
            >
              {strings.eqViewVisual}
            </button>
          </div>
          <button
            type="button"
            className={styles.close}
            onClick={onClose}
            aria-label={strings.eqClose}
          >
            <X size={18} />
          </button>
        </div>

        {view === "latex" && (
          <textarea
            ref={inputRef}
            className={styles.input}
            value={value}
            spellCheck={false}
            placeholder={strings.eqPlaceholder}
            onChange={(e) => onChange(e.target.value)}
            aria-label={strings.eqInputLabel}
            rows={1}
          />
        )}

        <div className={styles.previewWrap}>
          <span className={styles.previewLabel}>{strings.eqPreview}</span>
          <div className={styles.preview}>
            {rendered.error !== null ? (
              <span className={styles.error}>{strings.eqError(rendered.error)}</span>
            ) : value.trim().length === 0 ? (
              <span className={styles.empty}>{strings.eqEmpty}</span>
            ) : (
              <span
                className={styles.math}
                // KaTeX escapes its own input; `trust:false` blocks command injection.
                dangerouslySetInnerHTML={{ __html: rendered.html }}
              />
            )}
          </div>
        </div>

        <div className={styles.footer}>
          <div className={styles.symbols}>
            {SYMBOLS.map((s) => (
              <SymbolButton key={s.tip} symbol={s} onInsert={insert} />
            ))}
          </div>
          {onToggleNumbered !== undefined && (
            <label className={styles.numbered}>
              <input
                type="checkbox"
                checked={numbered ?? false}
                onChange={(e) => onToggleNumbered(e.target.checked)}
              />
              {strings.eqNumbered}
            </label>
          )}
          <button
            type="button"
            className={styles.insert}
            onClick={onInsert}
            disabled={rendered.error !== null || value.trim().length === 0}
          >
            {strings.eqInsert}
          </button>
        </div>
      </div>
    </div>
  );
}
