// A tokenized recipient field: existing recipients render as removable chips
// (avatar + name/email), and typing an address then Enter / comma / semicolon /
// Tab — or blurring — commits it as a new chip. Backspace on an empty input
// removes the last chip. Used for To / Cc / Bcc in the compose window.
import { useState } from "react";
import type { KeyboardEvent, ReactNode } from "react";
import { X } from "lucide-react";

import { strings } from "../../i18n";
import { Avatar } from "../../ds";
import type { EmailAddress } from "../../jmap";
import { senderName } from "../format";
import styles from "./RecipientInput.module.css";

interface RecipientInputProps {
  label: string;
  value: EmailAddress[];
  onChange: (next: EmailAddress[]) => void;
  autoFocus?: boolean;
  /** Extra controls rendered at the right of the row (e.g. Cc/Bcc toggles). */
  trailing?: ReactNode;
}

/** Split raw text into candidate addresses and keep the ones with an "@". */
function parseAddresses(text: string): EmailAddress[] {
  return text
    .split(/[,;\s]+/)
    .map((s) => s.trim())
    .filter((s) => s.includes("@"))
    .map((email) => ({ name: null, email }));
}

function chipLabel(a: EmailAddress): string {
  return a.name !== null && a.name.trim().length > 0 ? a.name : a.email;
}

export function RecipientInput({ label, value, onChange, autoFocus, trailing }: RecipientInputProps) {
  const [draft, setDraft] = useState("");

  function commit(text: string): boolean {
    const parsed = parseAddresses(text);
    if (parsed.length === 0) return false;
    const seen = new Set(value.map((a) => a.email.toLowerCase()));
    const added = parsed.filter((a) => {
      const key = a.email.toLowerCase();
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
    if (added.length > 0) onChange([...value, ...added]);
    return true;
  }

  function onKeyDown(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter" || e.key === "," || e.key === ";" || e.key === "Tab") {
      if (draft.trim().length > 0) {
        e.preventDefault();
        if (commit(draft)) setDraft("");
      }
    } else if (e.key === "Backspace" && draft.length === 0 && value.length > 0) {
      onChange(value.slice(0, -1));
    }
  }

  function remove(index: number) {
    onChange(value.filter((_, i) => i !== index));
  }

  return (
    <div className={styles.row}>
      <span className={styles.label}>{label}</span>
      <div className={styles.field}>
        {value.map((a, i) => (
          <span key={`${a.email}-${i}`} className={styles.chip}>
            <Avatar name={senderName({ from: [a] })} email={a.email} size="sm" />
            <span className={styles.chipLabel}>{chipLabel(a)}</span>
            <button
              type="button"
              className={styles.remove}
              onClick={() => remove(i)}
              aria-label={strings.removeRecipient(chipLabel(a))}
            >
              <X size={13} />
            </button>
          </span>
        ))}
        <input
          className={styles.input}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={onKeyDown}
          onBlur={() => {
            if (commit(draft)) setDraft("");
          }}
          autoFocus={autoFocus}
          aria-label={label}
        />
      </div>
      {trailing !== undefined && <div className={styles.trailing}>{trailing}</div>}
    </div>
  );
}
