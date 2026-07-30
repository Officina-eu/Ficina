// A prose block (ADR 0015): rendered text with inline math and cross-reference
// chips; click to edit in a textarea whose toolbar inserts inline math (`$…$`)
// and cross-references (`{{ref:ID}}`) at the caret. Cross-references are picked
// from the document's items so they always target a real block.
import { useEffect, useRef, useState } from "react";
import { FunctionSquare, Link2 } from "lucide-react";

import { strings } from "../i18n";
import type { DocItem, NumberInfo } from "./numbering";
import { renderProse } from "./prose";
import { CrossReferencePicker } from "./CrossReference";
import styles from "./ParagraphBlock.module.css";

interface ParagraphBlockProps {
  text: string;
  onChange: (text: string) => void;
  items: DocItem[];
  numbering: Map<string, NumberInfo>;
}

export function ParagraphBlock({ text, onChange, items, numbering }: ParagraphBlockProps) {
  const [editing, setEditing] = useState(false);
  const [refOpen, setRefOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (!editing) return;
    function onDown(e: MouseEvent) {
      if (containerRef.current !== null && !containerRef.current.contains(e.target as Node)) {
        setEditing(false);
        setRefOpen(false);
      }
    }
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [editing]);

  function insertAtCaret(snippet: string, caretBack = 0) {
    const el = inputRef.current;
    const start = el?.selectionStart ?? text.length;
    const end = el?.selectionEnd ?? text.length;
    const next = text.slice(0, start) + snippet + text.slice(end);
    onChange(next);
    const caret = start + snippet.length - caretBack;
    requestAnimationFrame(() => {
      el?.focus();
      el?.setSelectionRange(caret, caret);
    });
  }

  if (!editing) {
    return (
      <div
        className={styles.rendered}
        role="textbox"
        tabIndex={0}
        onClick={() => setEditing(true)}
        onFocus={() => setEditing(true)}
      >
        {text.trim().length === 0 ? (
          <span className={styles.placeholder}>{strings.paraPlaceholder}</span>
        ) : (
          renderProse(text, numbering)
        )}
      </div>
    );
  }

  return (
    <div className={styles.editor} ref={containerRef}>
      <div className={styles.toolbar}>
        <button type="button" className={styles.tool} onClick={() => insertAtCaret("$  $", 2)}>
          <FunctionSquare size={14} />
          {strings.paraInlineMath}
        </button>
        <div className={styles.refAnchor}>
          <button
            type="button"
            className={styles.tool}
            onClick={() => setRefOpen((v) => !v)}
          >
            <Link2 size={14} />
            {strings.paraReference}
          </button>
          {refOpen && (
            <div className={styles.refPop}>
              <CrossReferencePicker
                items={items}
                numbering={numbering}
                onPick={(id) => {
                  insertAtCaret(`{{ref:${id}}}`);
                  setRefOpen(false);
                }}
              />
            </div>
          )}
        </div>
      </div>
      <textarea
        ref={inputRef}
        className={styles.input}
        value={text}
        autoFocus
        placeholder={strings.paraPlaceholder}
        onChange={(e) => onChange(e.target.value)}
        aria-label={strings.paraLabel}
      />
      {text.trim().length > 0 && (
        <div className={styles.preview}>{renderProse(text, numbering)}</div>
      )}
    </div>
  );
}
