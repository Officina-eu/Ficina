// The compose window: a new message, reply, reply-all, or forward. Recipients
// are chips (To / Cc / Bcc), the quoted original is tucked behind a toggle, and
// on send it creates a draft (Email/set) then submits it (EmailSubmission/set),
// which sends it and files it to Sent. Bcc recipients ride the envelope only —
// never the visible headers.
import { useMemo, useRef, useState } from "react";
import type { FormEvent } from "react";
import { ArrowLeft, ArrowRight, Maximize2, Minimize2, Paperclip, Trash2, X } from "lucide-react";

import { strings } from "../../i18n";
import { Spinner, cx } from "../../ds";
import { useJmapClient } from "../../jmap";
import type { EmailAddress, EmailFull } from "../../jmap";
import { formatBytes, formatDate, senderName } from "../format";
import { textContent } from "../body";
import { RecipientInput } from "./RecipientInput";
import { RichTextEditor } from "./RichTextEditor";
import styles from "./ComposeModal.module.css";

interface PendingAttachment {
  blobId: string;
  name: string;
  type: string;
  size: number;
}

/** True when the composed HTML carries real formatting (not just line breaks),
 * so it's worth sending a text/html alternative. */
function hasFormatting(html: string): boolean {
  return /<(?:b|strong|i|em|u|a|ul|ol|li|h[1-6]|blockquote)\b/i.test(html);
}

/** A plain-text rendering of composed HTML, for the text/plain alternative. */
function htmlToText(html: string): string {
  const withBreaks = html
    .replace(/<\/(?:div|p|li|h[1-6]|blockquote)>/gi, "\n")
    .replace(/<br\s*\/?>/gi, "\n");
  const el = document.createElement("div");
  el.innerHTML = withBreaks;
  return (el.textContent ?? "").replace(/\n{3,}/g, "\n\n").trim();
}

/** Escape text for safe inclusion in an HTML body. */
function escapeHtml(text: string): string {
  const el = document.createElement("div");
  el.textContent = text;
  return el.innerHTML;
}

export interface ComposeContext {
  mode: "new" | "reply" | "replyAll" | "forward";
  /** The source message for a reply or forward. */
  replyTo?: EmailFull;
}

interface ComposeModalProps {
  context: ComposeContext;
  fromEmail: string;
  fromName: string;
  draftsMailboxId: string | null;
  onClose: () => void;
  onSent: () => void;
}

interface Prefill {
  to: EmailAddress[];
  cc: EmailAddress[];
  subject: string;
  /** The new message text (empty for a reply/forward — the user writes it). */
  body: string;
  /** The quoted original / forwarded block, shown behind a toggle and appended
   * to the body on send. */
  quoted: string;
  inReplyTo: string[];
  references: string[];
  showCc: boolean;
}

const EMPTY: Prefill = {
  to: [],
  cc: [],
  subject: "",
  body: "",
  quoted: "",
  inReplyTo: [],
  references: [],
  showCc: false,
};

/** Dedupe addresses, dropping empties and any already-seen (case-insensitive)
 * — `seen` is seeded with the addresses to exclude (e.g. the signed-in user). */
function collect(addrs: EmailAddress[], seen: Set<string>): EmailAddress[] {
  const out: EmailAddress[] = [];
  for (const a of addrs) {
    const key = a.email.trim().toLowerCase();
    if (key.length === 0 || seen.has(key)) continue;
    seen.add(key);
    out.push(a);
  }
  return out;
}

/** Reply-all recipients: To gets the sender plus every original To recipient;
 * Cc keeps the original Cc. The signed-in user (`me`) and any duplicate address
 * are removed throughout, and a Cc already present in To is dropped. Pure and
 * exported for testing. */
export function replyAllRecipients(
  source: Pick<EmailFull, "from" | "to" | "cc">,
  me: string,
): { to: EmailAddress[]; cc: EmailAddress[] } {
  const seen = new Set<string>([me.trim().toLowerCase()]);
  const to = collect([...(source.from ?? []), ...(source.to ?? [])], seen);
  const cc = collect(source.cc ?? [], seen);
  return { to, cc };
}

/** The "On <date>, <sender> wrote:" quoted-reply block. */
function quoteBlock(replyTo: EmailFull): string {
  const original = textContent(replyTo) ?? replyTo.preview;
  const header = `${formatDate(replyTo.receivedAt)} — ${senderName(replyTo)} ${strings.composeWroteOn}`;
  const quoted = original
    .split("\n")
    .map((line) => `> ${line}`)
    .join("\n");
  return `${header}\n${quoted}`;
}

/** The "---------- Forwarded message ----------" block. */
function forwardBlock(source: EmailFull): string {
  const original = textContent(source) ?? source.preview;
  const recipients = (source.to ?? [])
    .map((a) => (a.name !== null && a.name.length > 0 ? a.name : a.email))
    .join(", ");
  return [
    strings.composeForwardedIntro,
    `${strings.composeLabelFrom} ${senderName(source)} <${source.from?.[0]?.email ?? ""}>`,
    `${strings.composeLabelDate} ${formatDate(source.receivedAt)}`,
    `${strings.composeLabelSubject} ${source.subject ?? ""}`,
    `${strings.composeLabelTo} ${recipients}`,
    "",
    original,
  ].join("\n");
}

function stripRe(subject: string | null, prefix: RegExp): string {
  return (subject ?? "").replace(prefix, "");
}

function buildPrefill(context: ComposeContext, me: string): Prefill {
  const src = context.replyTo;
  if (src === undefined) return EMPTY;
  const threading = {
    inReplyTo: src.messageId ?? [],
    references: [...(src.references ?? []), ...(src.messageId ?? [])],
  };
  const firstFrom = src.from?.[0] !== undefined ? [src.from[0]] : [];

  if (context.mode === "reply") {
    return {
      ...EMPTY,
      ...threading,
      to: firstFrom,
      subject: strings.composeReplyPrefix + stripRe(src.subject, /^(re:\s*)+/i),
      quoted: quoteBlock(src),
    };
  }
  if (context.mode === "replyAll") {
    const { to, cc } = replyAllRecipients(src, me);
    return {
      ...EMPTY,
      ...threading,
      to,
      cc,
      showCc: cc.length > 0,
      subject: strings.composeReplyPrefix + stripRe(src.subject, /^(re:\s*)+/i),
      quoted: quoteBlock(src),
    };
  }
  if (context.mode === "forward") {
    // Forwarding starts a fresh conversation — no threading headers.
    return {
      ...EMPTY,
      subject: strings.composeForwardPrefix + stripRe(src.subject, /^(fwd:\s*)+/i),
      quoted: forwardBlock(src),
    };
  }
  return EMPTY;
}

function title(mode: ComposeContext["mode"]): string {
  switch (mode) {
    case "reply":
      return strings.composeReplyTitle;
    case "replyAll":
      return strings.composeReplyAllTitle;
    case "forward":
      return strings.composeForwardTitle;
    default:
      return strings.composeTitle;
  }
}

export function ComposeModal({
  context,
  fromEmail,
  fromName,
  draftsMailboxId,
  onClose,
  onSent,
}: ComposeModalProps) {
  const client = useJmapClient();
  const prefill = useMemo(() => buildPrefill(context, fromEmail), [context, fromEmail]);
  const isReply = context.mode === "reply" || context.mode === "replyAll";

  const [to, setTo] = useState<EmailAddress[]>(prefill.to);
  const [cc, setCc] = useState<EmailAddress[]>(prefill.cc);
  const [bcc, setBcc] = useState<EmailAddress[]>([]);
  const [showCc, setShowCc] = useState(prefill.showCc);
  const [showBcc, setShowBcc] = useState(false);
  const [subject, setSubject] = useState(prefill.subject);
  const [body, setBody] = useState(prefill.body);
  const [showQuoted, setShowQuoted] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [attachments, setAttachments] = useState<PendingAttachment[]>([]);
  const [uploading, setUploading] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const fileInput = useRef<HTMLInputElement>(null);

  async function onPickFiles(files: FileList) {
    setError(null);
    for (const file of Array.from(files)) {
      setUploading((n) => n + 1);
      try {
        const up = await client.uploadFile(file);
        setAttachments((prev) => [
          ...prev,
          { blobId: up.blobId, name: file.name, type: up.type, size: up.size },
        ]);
      } catch {
        setError(strings.attachmentUploadFailed);
      } finally {
        setUploading((n) => n - 1);
      }
    }
  }

  function removeAttachment(blobId: string) {
    setAttachments((prev) => prev.filter((a) => a.blobId !== blobId));
  }

  const recipientTotal = useMemo(() => {
    const seen = new Set<string>();
    for (const a of [...to, ...cc, ...bcc]) seen.add(a.email.toLowerCase());
    return seen.size;
  }, [to, cc, bcc]);

  async function onSend(event: FormEvent) {
    event.preventDefault();
    if (to.length === 0 && cc.length === 0 && bcc.length === 0) {
      setError(strings.composeNoRecipients);
      return;
    }
    if (draftsMailboxId === null) {
      setError(strings.composeSendError);
      return;
    }
    setSending(true);
    setError(null);
    // The editor holds HTML; derive the text/plain alternative and append the
    // quoted original (as text, and as HTML when we're sending a formatted body).
    const text = htmlToText(body);
    const fullText = prefill.quoted.length > 0 ? `${text}\n\n${prefill.quoted}` : text;
    let bodyHtml: string | undefined;
    if (hasFormatting(body)) {
      const quotedHtml =
        prefill.quoted.length > 0
          ? `<br><br><blockquote>${escapeHtml(prefill.quoted).replace(/\n/g, "<br>")}</blockquote>`
          : "";
      bodyHtml = `${body}${quotedHtml}`;
    }
    try {
      const emailId = await client.createDraft({
        mailboxId: draftsMailboxId,
        from: { name: fromName.length > 0 ? fromName : null, email: fromEmail },
        to,
        cc,
        subject,
        bodyText: fullText,
        ...(bodyHtml !== undefined ? { bodyHtml } : {}),
        inReplyTo: prefill.inReplyTo,
        references: prefill.references,
        attachments: attachments.map((a) => ({ blobId: a.blobId, type: a.type, name: a.name })),
      });
      // Bcc rides the envelope only — it is deliberately absent from the draft
      // headers above but present in the submission recipients here.
      const rcpts = [...to, ...cc, ...bcc].map((a) => a.email);
      await client.submitEmail(emailId, fromEmail, rcpts);
      onSent();
    } catch {
      setError(strings.composeSendError);
      setSending(false);
    }
  }

  return (
    <div className={styles.overlay} onMouseDown={onClose}>
      <div
        className={cx(styles.modal, expanded && styles.expanded)}
        role="dialog"
        aria-modal="true"
        aria-label={title(context.mode)}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <form onSubmit={onSend} className={styles.form}>
          <header className={styles.head}>
            <button type="button" className={styles.iconBtn} onClick={onClose} aria-label={strings.composeBack}>
              <ArrowLeft size={18} />
            </button>
            <h2 className={styles.title}>{title(context.mode)}</h2>
            {recipientTotal > 0 && (
              <span className={styles.countPill}>{strings.recipientCount(recipientTotal)}</span>
            )}
            <div className={styles.headSpacer} />
            <button
              type="button"
              className={styles.iconBtn}
              onClick={() => setExpanded((v) => !v)}
              aria-label={expanded ? strings.composeCollapse : strings.composeExpand}
            >
              {expanded ? <Minimize2 size={16} /> : <Maximize2 size={16} />}
            </button>
            <button type="button" className={styles.iconBtn} onClick={onClose} aria-label={strings.composeDiscard}>
              <X size={18} />
            </button>
          </header>

          <div className={styles.fields}>
            <RecipientInput
              label={strings.composeTo}
              value={to}
              onChange={setTo}
              autoFocus={!isReply}
              trailing={
                <>
                  {!showCc && (
                    <button type="button" className={styles.ccBtn} onClick={() => setShowCc(true)}>
                      {strings.composeCc}
                    </button>
                  )}
                  {!showBcc && (
                    <button type="button" className={styles.ccBtn} onClick={() => setShowBcc(true)}>
                      {strings.composeBcc}
                    </button>
                  )}
                </>
              }
            />
            {showCc && <RecipientInput label={strings.composeCc} value={cc} onChange={setCc} />}
            {showBcc && <RecipientInput label={strings.composeBcc} value={bcc} onChange={setBcc} />}
            <div className={styles.subjectRow}>
              <span className={styles.subjectLabel}>{strings.composeSubject}</span>
              <input
                className={styles.subjectInput}
                value={subject}
                onChange={(e) => setSubject(e.target.value)}
                placeholder={strings.composeSubjectPlaceholder}
              />
            </div>
          </div>

          <RichTextEditor
            initialHtml={prefill.body}
            onChange={setBody}
            placeholder={strings.composeBodyPlaceholder}
            autoFocus={isReply}
          />

          {prefill.quoted.length > 0 && (
            <div className={styles.quotedWrap}>
              <button
                type="button"
                className={styles.quotedToggle}
                onClick={() => setShowQuoted((v) => !v)}
              >
                {showQuoted ? strings.hideQuoted : strings.showQuoted}
              </button>
              {showQuoted && <pre className={styles.quoted}>{prefill.quoted}</pre>}
            </div>
          )}

          {(attachments.length > 0 || uploading > 0) && (
            <div className={styles.attachRow}>
              {attachments.map((a) => (
                <span key={a.blobId} className={styles.attachChip}>
                  <Paperclip size={14} className={styles.attachIcon} />
                  <span className={styles.attachName}>{a.name}</span>
                  <span className={styles.attachSize}>{formatBytes(a.size)}</span>
                  <button
                    type="button"
                    className={styles.attachRemove}
                    onClick={() => removeAttachment(a.blobId)}
                    aria-label={strings.removeRecipient(a.name)}
                  >
                    <X size={13} />
                  </button>
                </span>
              ))}
              {uploading > 0 && (
                <span className={styles.attachChip}>
                  <Spinner size={14} />
                  <span className={styles.attachName}>{strings.attachmentUploading}</span>
                </span>
              )}
            </div>
          )}

          {error !== null && (
            <p className={styles.error} role="alert">
              {error}
            </p>
          )}

          <footer className={styles.footer}>
            <button
              type="button"
              className={styles.discard}
              onClick={onClose}
              aria-label={strings.composeDiscard}
            >
              <Trash2 size={17} />
            </button>
            <button
              type="button"
              className={styles.iconBtn}
              onClick={() => fileInput.current?.click()}
              aria-label={strings.attach}
            >
              <Paperclip size={18} />
            </button>
            <input
              ref={fileInput}
              type="file"
              multiple
              className={styles.fileInput}
              onChange={(e) => {
                if (e.target.files !== null && e.target.files.length > 0) {
                  void onPickFiles(e.target.files);
                }
                e.target.value = "";
              }}
            />
            <div className={styles.headSpacer} />
            <button type="submit" className={styles.send} disabled={sending || uploading > 0}>
              {sending ? (
                <Spinner size={16} label={strings.composeSending} />
              ) : (
                <>
                  <span>{strings.composeSend}</span>
                  <ArrowRight size={16} />
                </>
              )}
            </button>
          </footer>
        </form>
      </div>
    </div>
  );
}
