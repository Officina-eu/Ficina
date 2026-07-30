// A lightweight rich-text editor for compose: a contentEditable surface with a
// Bold / Italic / Underline / Link toolbar. It stays dependency-free — the
// formatting uses the browser's built-in editing commands — and emits HTML on
// every edit. The parent derives a plain-text alternative from that HTML.
import { Suspense, lazy, useEffect, useRef, useState } from "react";
import { Bold, Code2, Italic, Link2, Sigma, Underline } from "lucide-react";

import { strings } from "../../i18n";
import styles from "./RichTextEditor.module.css";

// The equation/code insert UI pulls in KaTeX + Prism, so it is code-split: those
// libraries load only when a user inserts one, never on the mail path (ADR 0015).
const AuthoringInsertModal = lazy(() =>
  import("../../authoring").then((m) => ({ default: m.AuthoringInsertModal })),
);

interface RichTextEditorProps {
  /** Initial HTML (uncontrolled thereafter — set once on mount). */
  initialHtml: string;
  /** Called with the editor's current HTML on every edit. */
  onChange: (html: string) => void;
  placeholder: string;
  autoFocus?: boolean;
}

type Command = "bold" | "italic" | "underline";

export function RichTextEditor({ initialHtml, onChange, placeholder, autoFocus }: RichTextEditorProps) {
  const ref = useRef<HTMLDivElement>(null);
  const savedRange = useRef<Range | null>(null);
  const [insert, setInsert] = useState<null | "equation" | "code">(null);

  // Seed the editor from the initial HTML. `initialHtml`/`autoFocus` are stable
  // (memoized by the parent), so this runs once; the editor is uncontrolled
  // afterwards, so the caret is never disturbed by re-renders.
  useEffect(() => {
    const el = ref.current;
    if (el === null) return;
    el.innerHTML = initialHtml;
    if (autoFocus === true) el.focus();
  }, [initialHtml, autoFocus]);

  function emit() {
    onChange(ref.current?.innerHTML ?? "");
  }

  function apply(command: Command) {
    ref.current?.focus();
    document.execCommand(command);
    emit();
  }

  function addLink() {
    const url = window.prompt(strings.linkPrompt);
    if (url === null || url.trim().length === 0) return;
    ref.current?.focus();
    document.execCommand("createLink", false, url.trim());
    emit();
  }

  // Remember where the caret is before the insert modal steals focus, so the
  // block lands where the user was typing.
  function openInsert(kind: "equation" | "code") {
    const sel = window.getSelection();
    savedRange.current =
      sel !== null && sel.rangeCount > 0 && ref.current?.contains(sel.getRangeAt(0).commonAncestorContainer)
        ? sel.getRangeAt(0).cloneRange()
        : null;
    setInsert(kind);
  }

  function insertHtml(html: string) {
    setInsert(null);
    const el = ref.current;
    if (el === null) return;
    el.focus();
    const sel = window.getSelection();
    if (sel === null) return;

    // Insert at the saved caret (or the document end). Parse via <template> so
    // MathML and the code block survive intact — more reliable than execCommand.
    let range: Range;
    if (savedRange.current !== null) {
      range = savedRange.current;
    } else {
      range = document.createRange();
      range.selectNodeContents(el);
      range.collapse(false);
    }
    range.deleteContents();
    const tpl = document.createElement("template");
    // A trailing space keeps the caret editable after an atomic (math/code) block.
    tpl.innerHTML = `${html}&nbsp;`;
    const lastNode = tpl.content.lastChild;
    range.insertNode(tpl.content);
    if (lastNode !== null) {
      const after = document.createRange();
      after.setStartAfter(lastNode);
      after.collapse(true);
      sel.removeAllRanges();
      sel.addRange(after);
    }
    emit();
  }

  return (
    <div className={styles.wrap}>
      <div className={styles.toolbar} role="toolbar" aria-label={strings.formatting}>
        <button
          type="button"
          className={styles.tool}
          aria-label={strings.bold}
          title={strings.bold}
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => apply("bold")}
        >
          <Bold size={16} />
        </button>
        <button
          type="button"
          className={styles.tool}
          aria-label={strings.italic}
          title={strings.italic}
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => apply("italic")}
        >
          <Italic size={16} />
        </button>
        <button
          type="button"
          className={styles.tool}
          aria-label={strings.underline}
          title={strings.underline}
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => apply("underline")}
        >
          <Underline size={16} />
        </button>
        <button
          type="button"
          className={styles.tool}
          aria-label={strings.link}
          title={strings.link}
          onMouseDown={(e) => e.preventDefault()}
          onClick={addLink}
        >
          <Link2 size={16} />
        </button>
        <span className={styles.divider} aria-hidden="true" />
        <button
          type="button"
          className={styles.tool}
          aria-label={strings.composeInsertEquation}
          title={strings.composeInsertEquation}
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => openInsert("equation")}
        >
          <Sigma size={16} />
        </button>
        <button
          type="button"
          className={styles.tool}
          aria-label={strings.composeInsertCode}
          title={strings.composeInsertCode}
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => openInsert("code")}
        >
          <Code2 size={16} />
        </button>
      </div>
      <div
        ref={ref}
        className={styles.editor}
        contentEditable
        role="textbox"
        aria-multiline="true"
        aria-label={placeholder}
        data-placeholder={placeholder}
        onInput={emit}
        suppressContentEditableWarning
      />
      {insert !== null && (
        <Suspense fallback={null}>
          <AuthoringInsertModal
            kind={insert}
            onInsert={insertHtml}
            onClose={() => setInsert(null)}
          />
        </Suspense>
      )}
    </div>
  );
}
