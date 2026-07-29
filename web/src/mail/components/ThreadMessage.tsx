// One message within a conversation. Collapsed: a clickable summary row
// (avatar, sender, snippet, date). Expanded: the sender block plus the body —
// plain text in Garamond, HTML isolated in a sandboxed, CSP-locked iframe.
import { useState } from "react";
import { Download, Forward, Paperclip, Reply, ReplyAll, Trash2 } from "lucide-react";

import { strings } from "../../i18n";
import { Avatar, IconButton, Spinner, cx } from "../../ds";
import { useJmapClient } from "../../jmap";
import type { EmailAddress, EmailAttachment, EmailFull } from "../../jmap";
import { formatBytes, formatDate, senderName, subjectOr } from "../format";
import { htmlContent, sandboxedHtml, textContent } from "../body";
import styles from "./ThreadMessage.module.css";

/** Save a fetched Blob to the user's downloads with the given filename. */
function saveBlob(blob: Blob, name: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}

/** A downloadable attachment chip: fetches the bytes (authorized) on click and
 * saves them, showing a spinner while in flight. */
function AttachmentChip({ attachment }: { attachment: EmailAttachment }) {
  const client = useJmapClient();
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);

  async function download() {
    if (busy) return;
    setBusy(true);
    setFailed(false);
    try {
      const blob = await client.downloadAttachment(attachment.blobId, attachment.name);
      saveBlob(blob, attachment.name);
    } catch {
      setFailed(true);
    } finally {
      setBusy(false);
    }
  }

  return (
    <button
      type="button"
      className={cx(styles.attachment, failed && styles.attachmentFailed)}
      onClick={download}
      disabled={busy}
      title={failed ? strings.attachmentFailed : strings.downloadAttachment(attachment.name)}
    >
      <Paperclip className={styles.attachIcon} aria-hidden="true" />
      <span className={styles.attachName}>{attachment.name}</span>
      <span className={styles.attachSize}>{formatBytes(attachment.size)}</span>
      {busy ? (
        <Spinner size={14} />
      ) : (
        <Download className={styles.attachIcon} aria-hidden="true" />
      )}
    </button>
  );
}

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
  const attachments = expanded ? (email.attachments ?? []) : [];

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
          {text === null && html === null && attachments.length === 0 && (
            <p className={styles.empty}>{email.preview}</p>
          )}
          {attachments.length > 0 && (
            <div className={styles.attachments}>
              {attachments.map((a) => (
                <AttachmentChip key={a.blobId} attachment={a} />
              ))}
            </div>
          )}
        </div>
      )}
    </article>
  );
}
