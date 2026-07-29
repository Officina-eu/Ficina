// One message within a conversation. Collapsed: a clickable summary row
// (avatar, sender, snippet, date). Expanded: the sender block plus the body —
// plain text in Garamond, HTML isolated in a sandboxed, CSP-locked iframe.
import { Paperclip } from "lucide-react";

import { strings } from "../../i18n";
import { Avatar, cx } from "../../ds";
import type { EmailAddress, EmailFull } from "../../jmap";
import { formatDate, senderName, subjectOr } from "../format";
import { htmlContent, sandboxedHtml, textContent } from "../body";
import styles from "./ThreadMessage.module.css";

interface ThreadMessageProps {
  email: EmailFull;
  expanded: boolean;
  /** The signed-in user's address, so their own line reads "me". */
  me: string | undefined;
  onToggle: () => void;
}

function recipientLine(to: EmailAddress[] | null, me: string | undefined): string {
  if (to === null || to.length === 0) return "";
  if (me !== undefined && to.some((a) => a.email.toLowerCase() === me)) return "me";
  return to.map((a) => (a.name !== null && a.name.length > 0 ? a.name : a.email)).join(", ");
}

export function ThreadMessage({ email, expanded, me, onToggle }: ThreadMessageProps) {
  const text = expanded ? textContent(email) : null;
  const html = expanded && text === null ? htmlContent(email) : null;

  return (
    <article className={cx(styles.message, expanded && styles.expanded)}>
      <button type="button" className={styles.head} onClick={onToggle} aria-expanded={expanded}>
        <span className={styles.node}>
          <Avatar name={senderName(email)} email={email.from?.[0]?.email} size="md" />
        </span>
        <div className={styles.headText}>
          <div className={styles.headTop}>
            <span className={styles.sender}>{senderName(email)}</span>
            {email.hasAttachment && <Paperclip className={styles.clip} aria-hidden="true" />}
            <span className={styles.date}>{formatDate(email.receivedAt)}</span>
          </div>
          <div className={styles.headSub}>
            {expanded
              ? `${email.from?.[0]?.email ?? ""} · ${strings.toLabel} ${recipientLine(email.to, me)}`
              : email.preview}
          </div>
        </div>
      </button>

      {expanded && (
        <div className={styles.body}>
          {text !== null && <pre className={styles.text}>{text}</pre>}
          {html !== null && (
            <iframe
              className={styles.html}
              title={subjectOr(email)}
              sandbox=""
              srcDoc={sandboxedHtml(html)}
            />
          )}
          {text === null && html === null && <p className={styles.empty}>{email.preview}</p>}
        </div>
      )}
    </article>
  );
}
