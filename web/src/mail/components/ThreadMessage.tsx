// One message within a conversation. Collapsed: a clickable summary row
// (avatar, sender, snippet, date). Expanded: the sender block plus the body —
// plain text in Garamond, HTML isolated in a sandboxed, CSP-locked iframe.
import { useState } from "react";
import { Download, Paperclip, ShieldCheck } from "lucide-react";

import { strings } from "../../i18n";
import { Avatar, Spinner, cx } from "../../ds";
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
}

function displayName(a: EmailAddress, me: string | undefined): string {
  if (me !== undefined && a.email.toLowerCase() === me) return "me";
  return a.name !== null && a.name.length > 0 ? a.name : a.email;
}

function recipientLine(to: EmailAddress[] | null, me: string | undefined): string {
  if (to === null || to.length === 0) return "";
  return to.map((a) => displayName(a, me)).join(", ");
}

/** True when inbound authentication passed strongly enough to vouch for the
 * sender: DMARC pass, or DKIM pass in the absence of a DMARC verdict. */
function isVerified(email: EmailFull): boolean {
  const auth = email["ficina:authentication"];
  if (auth === undefined || auth === null) return false;
  if (auth.dmarc === "pass") return true;
  return auth.dkim === "pass" && (auth.dmarc === null || auth.dmarc === "none");
}

/** One "To / Cc / Bcc" row of the expanded recipient block; renders nothing
 * when the field is empty. Bcc is only ever populated on the sender's own copy. */
function RecipientRow({
  label,
  people,
  me,
}: {
  label: string;
  people: EmailAddress[] | null;
  me: string | undefined;
}) {
  if (people === null || people.length === 0) return null;
  return (
    <div className={styles.recipientRow}>
      <span className={styles.recipientLabel}>{label}</span>
      <span className={styles.recipientNames}>{recipientLine(people, me)}</span>
    </div>
  );
}

export function ThreadMessage({ email, expanded, me, onToggle }: ThreadMessageProps) {
  const text = expanded ? textContent(email) : null;
  const html = expanded && text === null ? htmlContent(email) : null;
  const attachments = expanded ? (email.attachments ?? []) : [];
  const verified = expanded && isVerified(email);

  return (
    <article className={cx(styles.message, expanded && styles.expanded)}>
      <button type="button" className={styles.head} onClick={onToggle} aria-expanded={expanded}>
        <Avatar name={senderName(email)} email={email.from?.[0]?.email} size="md" />
        <div className={styles.headText}>
          <div className={styles.headTop}>
            <span className={styles.sender}>{senderName(email)}</span>
            {expanded &&
              email.from?.[0]?.email !== undefined &&
              email.from[0].email !== senderName(email) && (
                <span className={styles.senderEmail}>{`<${email.from[0].email}>`}</span>
              )}
            {verified && (
              <span className={styles.verified} title={strings.senderVerifiedTitle}>
                <ShieldCheck className={styles.verifiedIcon} aria-hidden="true" />
                {strings.senderVerified}
              </span>
            )}
            {email.hasAttachment && <Paperclip className={styles.clip} aria-hidden="true" />}
            <span className={styles.date}>{formatDate(email.receivedAt)}</span>
          </div>
          {expanded ? (
            <div className={styles.recipients}>
              <RecipientRow label={strings.toLabel} people={email.to} me={me} />
              <RecipientRow label={strings.ccLabel} people={email.cc} me={me} />
              <RecipientRow label={strings.bccLabel} people={email.bcc} me={me} />
            </div>
          ) : (
            <div className={styles.headSub}>{email.preview}</div>
          )}
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
