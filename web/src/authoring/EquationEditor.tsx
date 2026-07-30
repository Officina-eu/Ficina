// The equation editor (ADR 0015): a LaTeX source input with a live KaTeX preview,
// a LaTeX/Visual view toggle, inline vs numbered-display mode, and a common-symbol
// quick bar that inserts snippets at the caret. Rendering is browser-local; an
// invalid formula shows an inline error and never breaks the page.
import { useMemo, useRef, useState } from "react";

import { strings } from "../i18n";
import { cx } from "../ds";
import { renderMath } from "./katex";
import styles from "./EquationEditor.module.css";

/** One quick-bar entry: a KaTeX preview face, the snippet it inserts, and where
 * to drop the caret afterwards (offset from the end of the inserted snippet). */
interface Symbol {
  tip: string;
  preview: string;
  insert: string;
  /** Caret offset from the end of `insert` (negative = inside braces). */
  caret?: number;
}

const SYMBOLS: Symbol[] = [
  { tip: "Fraction", preview: "\\frac{a}{b}", insert: "\\frac{}{}", caret: -3 },
  { tip: "Superscript", preview: "x^{2}", insert: "^{}", caret: -1 },
  { tip: "Subscript", preview: "x_{i}", insert: "_{}", caret: -1 },
  { tip: "Square root", preview: "\\sqrt{x}", insert: "\\sqrt{}", caret: -1 },
  { tip: "Sum", preview: "\\sum_{i}^{n}", insert: "\\sum_{}^{}", caret: -3 },
  { tip: "Integral", preview: "\\int_{a}^{b}", insert: "\\int_{}^{}", caret: -3 },
  { tip: "Greek alpha", preview: "\\alpha", insert: "\\alpha " },
  { tip: "Greek beta", preview: "\\beta", insert: "\\beta " },
  { tip: "Greek pi", preview: "\\pi", insert: "\\pi " },
  { tip: "Greek theta", preview: "\\theta", insert: "\\theta " },
  { tip: "Infinity", preview: "\\infty", insert: "\\infty " },
  { tip: "Multiply", preview: "\\times", insert: "\\times " },
  { tip: "Less or equal", preview: "\\le", insert: "\\le " },
  { tip: "Greater or equal", preview: "\\ge", insert: "\\ge " },
  { tip: "Not equal", preview: "\\neq", insert: "\\neq " },
  { tip: "Approximately", preview: "\\approx", insert: "\\approx " },
  { tip: "Arrow", preview: "\\rightarrow", insert: "\\rightarrow " },
  { tip: "Partial", preview: "\\partial", insert: "\\partial " },
];

function SymbolButton({ symbol, onInsert }: { symbol: Symbol; onInsert: (s: Symbol) => void }) {
  const face = useMemo(() => renderMath(symbol.preview, false), [symbol.preview]);
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
  /** The LaTeX source. */
  value: string;
  onChange: (latex: string) => void;
  /** Display (block) equation vs inline math. */
  display: boolean;
  onDisplayChange: (display: boolean) => void;
  /** A numbered display equation; the number is shown as e.g. "(3)". */
  numbered: boolean;
  onNumberedChange: (numbered: boolean) => void;
  /** The equation's current number (from the numbering engine), shown when numbered. */
  number: string | undefined;
}

export function EquationEditor({
  value,
  onChange,
  display,
  onDisplayChange,
  numbered,
  onNumberedChange,
  number,
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
    // Restore focus and drop the caret at the requested spot inside the snippet.
    const caret = start + symbol.insert.length + (symbol.caret ?? 0);
    requestAnimationFrame(() => {
      el?.focus();
      el?.setSelectionRange(caret, caret);
    });
  }

  const showNumber = display && numbered && number !== undefined;

  return (
    <div className={styles.editor}>
      <div className={styles.toolbar}>
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
        <div className={styles.spacer} />
        <div className={styles.segment} role="group" aria-label={strings.eqModeLabel}>
          <button
            type="button"
            className={cx(styles.segBtn, !display && styles.segOn)}
            onClick={() => onDisplayChange(false)}
          >
            {strings.eqInline}
          </button>
          <button
            type="button"
            className={cx(styles.segBtn, display && styles.segOn)}
            onClick={() => onDisplayChange(true)}
          >
            {strings.eqDisplay}
          </button>
        </div>
        <label className={cx(styles.numbered, !display && styles.disabled)}>
          <input
            type="checkbox"
            checked={numbered}
            disabled={!display}
            onChange={(e) => onNumberedChange(e.target.checked)}
          />
          {strings.eqNumbered}
        </label>
      </div>

      {view === "latex" && (
        <>
          <div className={styles.symbols}>
            {SYMBOLS.map((s) => (
              <SymbolButton key={s.tip} symbol={s} onInsert={insert} />
            ))}
          </div>
          <textarea
            ref={inputRef}
            className={styles.input}
            value={value}
            spellCheck={false}
            placeholder={strings.eqPlaceholder}
            onChange={(e) => onChange(e.target.value)}
            aria-label={strings.eqInputLabel}
          />
        </>
      )}

      <div className={styles.previewWrap}>
        <span className={styles.previewLabel}>{strings.eqPreview}</span>
        <div className={cx(styles.preview, display ? styles.previewBlock : styles.previewInline)}>
          {rendered.error !== null ? (
            <span className={styles.error}>{strings.eqError(rendered.error)}</span>
          ) : value.trim().length === 0 ? (
            <span className={styles.empty}>{strings.eqEmpty}</span>
          ) : (
            <>
              <span
                className={styles.math}
                // KaTeX escapes its own input; `trust:false` blocks command injection.
                dangerouslySetInnerHTML={{ __html: rendered.html }}
              />
              {showNumber && <span className={styles.eqNumber}>{`(${number})`}</span>}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
