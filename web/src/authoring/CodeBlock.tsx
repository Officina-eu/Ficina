// A syntax-highlighted code block (ADR 0015): Prism highlighting behind an
// editable overlay, line numbers, a searchable language picker (explicit, never
// auto-detected), and a copy button. Highlighting is browser-local; the source
// never leaves the client.
import { useEffect, useMemo, useRef, useState } from "react";
import { Check, ChevronDown, Copy, Search } from "lucide-react";

import { strings } from "../i18n";
import { cx } from "../ds";
import { LANGUAGES, highlight, languageLabel } from "./prism";
import styles from "./CodeBlock.module.css";

/** The searchable language dropdown. */
function LanguagePicker({
  language,
  onChange,
}: {
  language: string;
  onChange: (id: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    function onDoc(e: MouseEvent) {
      if (ref.current !== null && !ref.current.contains(e.target as Node)) setOpen(false);
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") setOpen(false);
    }
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const matches = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (q.length === 0) return LANGUAGES;
    return LANGUAGES.filter((l) => l.label.toLowerCase().includes(q) || l.id.includes(q));
  }, [query]);

  return (
    <div className={styles.picker} ref={ref}>
      <button
        type="button"
        className={styles.pickerButton}
        onClick={() => setOpen((v) => !v)}
        aria-haspopup="listbox"
        aria-expanded={open}
      >
        {languageLabel(language)}
        <ChevronDown size={13} />
      </button>
      {open && (
        <div className={styles.pickerPanel} role="listbox">
          <div className={styles.pickerSearch}>
            <Search size={14} className={styles.searchIcon} />
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
                  onClick={() => {
                    onChange(l.id);
                    setOpen(false);
                    setQuery("");
                  }}
                >
                  {l.label}
                  {l.id === language && <Check size={14} />}
                </button>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
}

interface CodeBlockProps {
  code: string;
  onChange: (code: string) => void;
  language: string;
  onLanguageChange: (id: string) => void;
  /** Optional filename shown on the left of the header. */
  filename?: string;
}

export function CodeBlock({ code, onChange, language, onLanguageChange, filename }: CodeBlockProps) {
  const [copied, setCopied] = useState(false);
  const html = useMemo(() => highlight(code, language), [code, language]);
  const lineCount = useMemo(() => code.split("\n").length, [code]);

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
        {filename !== undefined && <span className={styles.filename}>{filename}</span>}
        <div className={styles.spacer} />
        <LanguagePicker language={language} onChange={onLanguageChange} />
        <button type="button" className={styles.copy} onClick={copy}>
          {copied ? <Check size={14} /> : <Copy size={14} />}
          {copied ? strings.codeCopied : strings.codeCopy}
        </button>
      </div>
      <div className={styles.body}>
        <div className={styles.gutter} aria-hidden="true">
          {Array.from({ length: lineCount }, (_, i) => (
            <span key={i} className={styles.lineNo}>
              {i + 1}
            </span>
          ))}
        </div>
        <div className={styles.codeCell}>
          <pre className={styles.pre} aria-hidden="true">
            <code className={`language-${language}`} dangerouslySetInnerHTML={{ __html: html }} />
          </pre>
          <textarea
            className={styles.input}
            value={code}
            spellCheck={false}
            wrap="off"
            onChange={(e) => onChange(e.target.value)}
            onKeyDown={onKeyDown}
            aria-label={strings.codeInputLabel}
          />
        </div>
      </div>
    </div>
  );
}
