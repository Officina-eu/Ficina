// A syntax-highlighted code block (ADR 0015), styled to the Figma Docs screen:
// a dark block with a language pill and a Copy button in the header, line
// numbers, and Prism highlighting behind an editable overlay. The language
// picker is a light dropdown with a search field and colored language badges —
// explicit choice, never auto-detected. Highlighting is browser-local.
import { useEffect, useMemo, useRef, useState } from "react";
import { Check, ChevronDown, Copy, Search, WrapText } from "lucide-react";

import { strings } from "../i18n";
import { cx } from "../ds";
import { LANGUAGES, highlight, languageLabel } from "./prism";
import { detectLanguage } from "./detectLanguage";
import styles from "./CodeBlock.module.css";

/** The searchable language dropdown with colored badges. */
function LanguagePicker({
  language,
  onChange,
  onClose,
}: {
  language: string;
  onChange: (id: string) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function onDoc(e: MouseEvent) {
      if (ref.current !== null && !ref.current.contains(e.target as Node)) onClose();
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [onClose]);

  const matches = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (q.length === 0) return LANGUAGES;
    return LANGUAGES.filter((l) => l.label.toLowerCase().includes(q) || l.id.includes(q));
  }, [query]);

  return (
    <div className={styles.pickerPanel} role="listbox" ref={ref}>
      <div className={styles.pickerSearch}>
        <Search size={15} className={styles.searchIcon} />
        <input
          autoFocus
          className={styles.searchInput}
          placeholder={strings.codeSearchLanguage}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </div>
      <div className={styles.pickerList}>
        {matches.length === 0 ? (
          <div className={styles.pickerEmpty}>{strings.codeNoLanguage}</div>
        ) : (
          matches.map((l) => (
            <button
              key={l.id}
              type="button"
              role="option"
              aria-selected={l.id === language}
              className={cx(styles.pickerItem, l.id === language && styles.pickerItemOn)}
              onClick={() => onChange(l.id)}
            >
              <span className={styles.badge} style={{ background: l.badgeBg }}>
                {l.badge}
              </span>
              <span className={styles.pickerLabel}>{l.label}</span>
              {l.id === language && <Check size={15} className={styles.pickerCheck} />}
            </button>
          ))
        )}
      </div>
    </div>
  );
}

interface CodeBlockProps {
  code: string;
  onChange: (code: string) => void;
  language: string;
  onLanguageChange: (id: string) => void;
  /** Give the editor a comfortable minimum height (the insert modal). */
  tall?: boolean;
}

export function CodeBlock({ code, onChange, language, onLanguageChange, tall }: CodeBlockProps) {
  const [copied, setCopied] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [wrap, setWrap] = useState(false);
  const html = useMemo(() => highlight(code, language), [code, language]);
  const lineCount = useMemo(() => code.split("\n").length, [code]);

  // Auto-detect the language on a fresh paste (empty editor) — a guess the user
  // can still override with the picker.
  function onPaste(e: React.ClipboardEvent<HTMLTextAreaElement>) {
    if (code.trim().length > 0) return;
    const detected = detectLanguage(e.clipboardData.getData("text"));
    if (detected !== null) onLanguageChange(detected);
  }

  async function copy() {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard denied — leave the button unchanged rather than error.
    }
  }

  function onKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key !== "Tab") return;
    e.preventDefault();
    const el = e.currentTarget;
    const start = el.selectionStart;
    const end = el.selectionEnd;
    const next = code.slice(0, start) + "  " + code.slice(end);
    onChange(next);
    requestAnimationFrame(() => el.setSelectionRange(start + 2, start + 2));
  }

  return (
    <div className={styles.block}>
      <div className={styles.header}>
        <div className={styles.pickerAnchor}>
          <button
            type="button"
            className={styles.langPill}
            onClick={() => setPickerOpen((v) => !v)}
            aria-haspopup="listbox"
            aria-expanded={pickerOpen}
          >
            {languageLabel(language)}
            <ChevronDown size={13} />
          </button>
          {pickerOpen && (
            <LanguagePicker
              language={language}
              onChange={(id) => {
                onLanguageChange(id);
                setPickerOpen(false);
              }}
              onClose={() => setPickerOpen(false)}
            />
          )}
        </div>
        <div className={styles.spacer} />
        <button
          type="button"
          className={cx(styles.iconBtn, wrap && styles.iconBtnOn)}
          onClick={() => setWrap((v) => !v)}
          aria-pressed={wrap}
          title={strings.codeWrap}
          aria-label={strings.codeWrap}
        >
          <WrapText size={14} />
        </button>
        {code.length > 0 && (
          <button type="button" className={styles.copy} onClick={copy}>
            {copied ? <Check size={14} /> : <Copy size={14} />}
            {copied ? strings.codeCopied : strings.codeCopy}
          </button>
        )}
      </div>
      <div className={styles.body}>
        {!wrap && (
          <div className={styles.gutter} aria-hidden="true">
            {Array.from({ length: lineCount }, (_, i) => (
              <span key={i} className={styles.lineNo}>
                {i + 1}
              </span>
            ))}
          </div>
        )}
        <div className={cx(styles.codeCell, tall && styles.tall, wrap && styles.wrapped)}>
          <pre className={styles.pre} aria-hidden="true">
            <code className={`language-${language}`} dangerouslySetInnerHTML={{ __html: html }} />
          </pre>
          <textarea
            className={styles.input}
            value={code}
            spellCheck={false}
            wrap={wrap ? "soft" : "off"}
            placeholder={strings.codePlaceholder}
            onChange={(e) => onChange(e.target.value)}
            onKeyDown={onKeyDown}
            onPaste={onPaste}
            aria-label={strings.codeInputLabel}
          />
        </div>
      </div>
    </div>
  );
}
