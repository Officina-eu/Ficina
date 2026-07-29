// A lightweight rich-text editor for compose: a contentEditable surface with a
// Bold / Italic / Underline / Link toolbar. It stays dependency-free — the
// formatting uses the browser's built-in editing commands — and emits HTML on
// every edit. The parent derives a plain-text alternative from that HTML.
import { useEffect, useRef } from "react";
import { Bold, Italic, Link2, Underline } from "lucide-react";

import { strings } from "../../i18n";
import styles from "./RichTextEditor.module.css";

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
    </div>
  );
}
