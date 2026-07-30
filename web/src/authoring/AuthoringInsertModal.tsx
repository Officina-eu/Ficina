// Insert-into-email modal (ADR 0015): lets compose insert an equation or a code
// block into the message body. Reuses the equation modal and the dark code
// editor, and emits email-safe HTML (MathML for math, inline-styled <pre> for
// code) that the compose editor drops in at the caret. Lazy-loaded, so KaTeX/
// Prism stay off the mail path until a user actually inserts one.
import { useState } from "react";

import { strings } from "../i18n";
import { EquationEditor } from "./EquationEditor";
import { CodeBlock } from "./CodeBlock";
import { DEFAULT_LANGUAGE } from "./prism";
import { codeEmailHtml, equationEmailHtml } from "./emailBlocks";
import styles from "./AuthoringInsertModal.module.css";

interface InsertProps {
  kind: "equation" | "code";
  /** Called with the email-safe HTML to insert at the caret. */
  onInsert: (html: string) => void;
  onClose: () => void;
}

function EquationInsert({ onInsert, onClose }: Omit<InsertProps, "kind">) {
  const [latex, setLatex] = useState("");
  return (
    <EquationEditor
      value={latex}
      onChange={setLatex}
      display={false}
      onInsert={() => onInsert(equationEmailHtml(latex, false))}
      onClose={onClose}
    />
  );
}

function CodeInsert({ onInsert, onClose }: Omit<InsertProps, "kind">) {
  const [code, setCode] = useState("");
  const [language, setLanguage] = useState(DEFAULT_LANGUAGE);
  return (
    <div className={styles.overlay} onMouseDown={onClose}>
      <div className={styles.modal} onMouseDown={(e) => e.stopPropagation()}>
        <div className={styles.head}>{strings.codeInsertTitle}</div>
        <CodeBlock
          code={code}
          onChange={setCode}
          language={language}
          onLanguageChange={setLanguage}
        />
        <div className={styles.footer}>
          <button type="button" className={styles.cancel} onClick={onClose}>
            {strings.insertCancel}
          </button>
          <button
            type="button"
            className={styles.insert}
            disabled={code.trim().length === 0}
            onClick={() => onInsert(codeEmailHtml(code, language))}
          >
            {strings.insertConfirm}
          </button>
        </div>
      </div>
    </div>
  );
}

export function AuthoringInsertModal({ kind, onInsert, onClose }: InsertProps) {
  return kind === "equation" ? (
    <EquationInsert onInsert={onInsert} onClose={onClose} />
  ) : (
    <CodeInsert onInsert={onInsert} onClose={onClose} />
  );
}
