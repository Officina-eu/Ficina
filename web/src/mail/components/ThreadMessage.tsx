// One message within a conversation. Collapsed: a clickable summary row
// (avatar, sender, snippet, date). Expanded: the sender block plus the body —
// plain text in Garamond, HTML isolated in a sandboxed, CSP-locked iframe.
import { Forward, Paperclip, Reply, ReplyAll, Trash2 } from "lucide-react";

import { strings } from "../../i18n";
import { Avatar, IconButton, cx } from "../../ds";
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
  /** Reply / reply-all / forward / delete this message (per-message action bar). */
  onReply: () => void;
  onReplyAll: () => void;
  onForward: () => void;
  onDelete: () => void;
}

function recipientLine(to: EmailAddress[] | null, me: string | undefined): string {
  if (to === null || to.length === 0) return "";
  if (me !== undefined && to.some((a) => a.email.toLowerCase() === me)) return "me";
  return to.map((a) => (a.name !== null && a.name.length > 0 ? a.name : a.email)).join(", ");
}

export function ThreadMessage({
  email,
  expanded,
  me,
  onToggle,
  onReply,
  onReplyAll,
  onForward,
  onDelete,
}: ThreadMessageProps) {
  const text = expanded ? textContent(email) : null;
  const html = expanded && text === null ? htmlContent(email) : null;

  return (
    <article className={cx(styles.message, expanded && styles.expanded)}>
      <button type="button" className={styles.head} onClick={onToggle} aria-expanded={expanded}>
        <Avatar name={senderName(email)} email={email.from?.[0]?.email} size="md" />
        <div className={styles.headText}>
          <div className={styles.headTop}>
            <span className={styles.sender}>{senderName(email)}</span>
            {expanded && email.from?.[0]?.email !== undefined && (
              <span className={styles.senderEmail}>{`<${email.from[0].email}>`}</span>
            )}
            {email.hasAttachment && <Paperclip className={styles.clip} aria-hidden="true" />}
            <span className={styles.date}>{formatDate(email.receivedAt)}</span>
          </div>
          <div className={styles.headSub}>
            {expanded ? `${strings.toLabel} ${recipientLine(email.to, me)}` : email.preview}
          </div>
        </div>
      </button>

      {expanded && (
        <div className={styles.body}>
          <div className={styles.msgActions}>
            <IconButton size="sm" label={strings.reply} icon={<Reply />} onClick={onReply} />
            <IconButton size="sm" label={strings.replyAll} icon={<ReplyAll />} onClick={onReplyAll} />
            <IconButton size="sm" label={strings.forward} icon={<Forward />} onClick={onForward} />
            <IconButton size="sm" label={strings.delete} icon={<Trash2 />} onClick={onDelete} />
          </div>
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
