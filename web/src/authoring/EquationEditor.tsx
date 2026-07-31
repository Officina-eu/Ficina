// The equation editor (ADR 0015): a Σ-marked panel with a LaTeX/Visual view
// toggle, a LaTeX source input, a live centered KaTeX preview, and an
// emoji-picker-style symbol browser — search across the full catalogue or
// browse by category. Rendering is browser-local; invalid LaTeX shows inline
// and never breaks the page.
import { useMemo, useRef, useState } from "react";
import { Search, Sigma, X } from "lucide-react";

import { strings } from "../i18n";
import { renderMath } from "./katex";
import { EQ_CATEGORIES, haystack, insertText, type EqSymbol } from "./equationSymbols";
import styles from "./EquationEditor.module.css";

/** Localised category headings, keyed by category id. */
const CAT_LABEL: Record<string, string> = {
  structures: strings.eqCatStructures,
  styles: strings.eqCatStyles,
  greek: strings.eqCatGreek,
  operators: strings.eqCatOperators,
  relations: strings.eqCatRelations,
  sets: strings.eqCatSets,
  arrows: strings.eqCatArrows,
  bigops: strings.eqCatBigops,
  calculus: strings.eqCatCalculus,
  delimiters: strings.eqCatDelimiters,
  misc: strings.eqCatMisc,
};

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
  const [query, setQuery] = useState("");
  const [searchOpen, setSearchOpen] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const catRefs = useRef(new Map<string, HTMLElement>());
  const rendered = useMemo(() => renderMath(value, display), [value, display]);

  // Flat search across the whole catalogue (name + command + keywords).
  const matches = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (q.length === 0) return null;
    const out: EqSymbol[] = [];
    for (const cat of EQ_CATEGORIES) {
      for (const s of cat.symbols) if (haystack(s).includes(q)) out.push(s);
    }
    return out;
  }, [query]);

  function scrollToCat(id: string) {
    catRefs.current.get(id)?.scrollIntoView({ block: "start", behavior: "smooth" });
  }

  function insert(symbol: EqSymbol) {
    const el = inputRef.current;
    const start = el?.selectionStart ?? value.length;
    const end = el?.selectionEnd ?? value.length;
    const ins = insertText(symbol);
    const next = value.slice(0, start) + ins + value.slice(end);
    onChange(next);
    const caret = start + ins.length + (symbol.caret ?? 0);
    requestAnimationFrame(() => {
      el?.focus();
      el?.setSelectionRange(caret, caret);
    });
  }

  function symbolButton(symbol: EqSymbol, key: string) {
    return (
      <button
        key={key}
        type="button"
        className={styles.symbol}
        title={`${symbol.name} · ${symbol.latex}`}
        aria-label={symbol.name}
        onClick={() => insert(symbol)}
      >
        {symbol.ch}
      </button>
    );
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
          <button
            type="button"
            className={styles.close}
            onClick={onClose}
            aria-label={strings.eqClose}
          >
            <X size={18} />
          </button>
        </div>

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

        <div className={styles.palette}>
          {searchOpen ? (
            <div className={styles.searchRow}>
              <Search size={16} className={styles.searchIcon} />
              <input
                type="text"
                className={styles.search}
                value={query}
                autoFocus
                onChange={(e) => setQuery(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Escape") {
                    setQuery("");
                    setSearchOpen(false);
                  }
                }}
                placeholder={strings.eqSearchPlaceholder}
                aria-label={strings.eqSearchLabel}
                spellCheck={false}
              />
              <button
                type="button"
                className={styles.searchClear}
                onClick={() => {
                  setQuery("");
                  setSearchOpen(false);
                }}
                aria-label={strings.eqSearchClear}
              >
                <X size={15} />
              </button>
            </div>
          ) : (
            <div className={styles.catRow}>
              <div className={styles.catNav} role="tablist" aria-label={strings.eqSearchLabel}>
                {EQ_CATEGORIES.map((c) => (
                  <button
                    key={c.id}
                    type="button"
                    className={styles.catChip}
                    onClick={() => scrollToCat(c.id)}
                  >
                    {CAT_LABEL[c.id]}
                  </button>
                ))}
              </div>
              <button
                type="button"
                className={styles.searchToggle}
                onClick={() => setSearchOpen(true)}
                aria-label={strings.eqSearchLabel}
                title={strings.eqSearchLabel}
              >
                <Search size={16} />
              </button>
            </div>
          )}

          <div className={styles.paletteScroll} ref={scrollRef}>
            {matches !== null ? (
              matches.length > 0 ? (
                <div className={styles.grid}>
                  {matches.map((s, i) => symbolButton(s, `r-${i}`))}
                </div>
              ) : (
                <p className={styles.noMatches}>{strings.eqNoMatches}</p>
              )
            ) : (
              EQ_CATEGORIES.map((c) => (
                <section
                  key={c.id}
                  className={styles.catSection}
                  ref={(el) => {
                    if (el !== null) catRefs.current.set(c.id, el);
                  }}
                >
                  <h4 className={styles.catHead}>{CAT_LABEL[c.id]}</h4>
                  <div className={styles.grid}>
                    {c.symbols.map((s, i) => symbolButton(s, `${c.id}-${i}`))}
                  </div>
                </section>
              ))
            )}
          </div>
        </div>

        <div className={styles.footer}>
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
          <div className={styles.spacer} />
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
