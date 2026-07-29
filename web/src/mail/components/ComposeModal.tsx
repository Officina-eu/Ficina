// The compose window: a new message or a reply. Recipients, subject, and a
// plain-text body; on send it creates a draft (Email/set) then submits it
// (EmailSubmission/set), which sends it and files it to Sent. Reply mode
// prefills the recipient, "Re:" subject, quoted original, and the threading
// headers (In-Reply-To / References).
import { useState } from "react";
import type { FormEvent } from "react";
import { X } from "lucide-react";

import { strings } from "../../i18n";
import { Button, Spinner } from "../../ds";
import { useJmapClient } from "../../jmap";
import type { EmailAddress, EmailFull } from "../../jmap";
import { formatDate, senderName } from "../format";
import { textContent } from "../body";
import styles from "./ComposeModal.module.css";

export interface ComposeContext {
  mode: "new" | "reply" | "replyAll" | "forward";
  /** The source message for a reply or forward. */
  replyTo?: EmailFull;
}

interface Prefill {
  to: string;
  cc: string;
  subject: string;
  body: string;
  inReplyTo: string[];
  references: string[];
}

interface ComposeModalProps {
  context: ComposeContext;
  fromEmail: string;
  fromName: string;
  draftsMailboxId: string | null;
  onClose: () => void;
  onSent: () => void;
}

function parseRecipients(input: string): EmailAddress[] {
  return input
    .split(/[,;]/)
    .map((s) => s.trim())
    .filter((s) => s.includes("@"))
    .map((email) => ({ name: null, email }));
}

function replyPrefill(replyTo: EmailFull): Prefill {
  const to = replyTo.from?.[0]?.email ?? "";
  const base = (replyTo.subject ?? "").replace(/^(re:\s*)+/i, "");
  const original = textContent(replyTo) ?? replyTo.preview;
  const header = `${formatDate(replyTo.receivedAt)} — ${senderName(replyTo)} ${strings.composeWroteOn}`;
  const quoted = original
    .split("\n")
    .map((line) => `> ${line}`)
    .join("\n");
  const messageIds = replyTo.messageId ?? [];
  return {
    to,
    cc: "",
    subject: strings.composeReplyPrefix + base,
    body: `\n\n${header}\n${quoted}\n`,
    inReplyTo: messageIds,
    references: [...(replyTo.references ?? []), ...messageIds],
  };
}

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

function replyAllPrefill(replyTo: EmailFull, me: string): Prefill {
  const { to, cc } = replyAllRecipients(replyTo, me);
  return {
    ...replyPrefill(replyTo),
    to: to.map((a) => a.email).join(", "),
    cc: cc.map((a) => a.email).join(", "),
  };
}

function forwardPrefill(source: EmailFull): Prefill {
  const base = (source.subject ?? "").replace(/^(fwd:\s*)+/i, "");
  const original = textContent(source) ?? source.preview;
  const recipients = (source.to ?? [])
    .map((a) => (a.name !== null && a.name.length > 0 ? a.name : a.email))
    .join(", ");
  const block = [
    "",
    "",
    strings.composeForwardedIntro,
    `${strings.composeLabelFrom} ${senderName(source)} <${source.from?.[0]?.email ?? ""}>`,
    `${strings.composeLabelDate} ${formatDate(source.receivedAt)}`,
    `${strings.composeLabelSubject} ${source.subject ?? ""}`,
    `${strings.composeLabelTo} ${recipients}`,
    "",
    original,
    "",
  ].join("\n");
  // Forwarding starts a fresh conversation — no reply threading headers.
  return {
    to: "",
    cc: "",
    subject: strings.composeForwardPrefix + base,
    body: block,
    inReplyTo: [],
    references: [],
  };
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
  const empty: Prefill = { to: "", cc: "", subject: "", body: "", inReplyTo: [], references: [] };
  const prefill: Prefill =
    context.replyTo === undefined
      ? empty
      : context.mode === "reply"
        ? replyPrefill(context.replyTo)
        : context.mode === "replyAll"
          ? replyAllPrefill(context.replyTo, fromEmail)
          : context.mode === "forward"
            ? forwardPrefill(context.replyTo)
            : empty;
  const isReply = context.mode === "reply" || context.mode === "replyAll";
  const title =
    context.mode === "replyAll"
      ? strings.replyAll
      : context.mode === "reply"
        ? strings.composeReplyTitle
        : context.mode === "forward"
          ? strings.composeForwardTitle
          : strings.composeTitle;

  const [to, setTo] = useState(prefill.to);
  const [cc, setCc] = useState(prefill.cc);
  const [showCc, setShowCc] = useState(prefill.cc.length > 0);
  const [subject, setSubject] = useState(prefill.subject);
  const [body, setBody] = useState(prefill.body);
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);

  async function onSend(event: FormEvent) {
    event.preventDefault();
    const toAddrs = parseRecipients(to);
    const ccAddrs = showCc ? parseRecipients(cc) : [];
    if (toAddrs.length === 0 && ccAddrs.length === 0) {
      setError(strings.composeNoRecipients);
      return;
    }
    if (draftsMailboxId === null) {
      setError(strings.composeSendError);
      return;
    }
    setSending(true);
    setError(null);
    try {
      const emailId = await client.createDraft({
        mailboxId: draftsMailboxId,
        from: { name: fromName.length > 0 ? fromName : null, email: fromEmail },
        to: toAddrs,
        cc: ccAddrs,
        subject,
        bodyText: body,
        inReplyTo: prefill.inReplyTo,
        references: prefill.references,
      });
      const rcpts = [...toAddrs, ...ccAddrs].map((a) => a.email);
      await client.submitEmail(emailId, fromEmail, rcpts);
      onSent();
    } catch {
      setError(strings.composeSendError);
    } finally {
      setSending(false);
    }
  }

  return (
    <div className={styles.overlay} onMouseDown={onClose}>
      <div
        className={styles.modal}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <form onSubmit={onSend} className={styles.form}>
          <div className={styles.head}>
            <h2 className={styles.title}>{title}</h2>
            <button
              type="button"
              className={styles.close}
              onClick={onClose}
              aria-label={strings.composeDiscard}
            >
              <X size={18} />
            </button>
          </div>

          <div className={styles.fields}>
            <div className={styles.field}>
              <span className={styles.label}>{strings.composeTo}</span>
              <input
                className={styles.input}
                value={to}
                onChange={(e) => setTo(e.target.value)}
                placeholder={strings.composeRecipientsPlaceholder}
                autoFocus={!isReply}
              />
              {!showCc && (
                <button type="button" className={styles.ccToggle} onClick={() => setShowCc(true)}>
                  {strings.composeCcToggle}
                </button>
              )}
            </div>
            {showCc && (
              <div className={styles.field}>
                <span className={styles.label}>{strings.composeCc}</span>
                <input
                  className={styles.input}
                  value={cc}
                  onChange={(e) => setCc(e.target.value)}
                  placeholder={strings.composeRecipientsPlaceholder}
                />
              </div>
            )}
            <div className={styles.field}>
              <span className={styles.label}>{strings.composeSubject}</span>
              <input
                className={styles.input}
                value={subject}
                onChange={(e) => setSubject(e.target.value)}
                placeholder={strings.composeSubjectPlaceholder}
              />
            </div>
          </div>

          <textarea
            className={styles.textarea}
            value={body}
            onChange={(e) => setBody(e.target.value)}
            placeholder={strings.composeBodyPlaceholder}
            autoFocus={isReply}
          />

          {error !== null && (
            <p className={styles.error} role="alert">
              {error}
            </p>
          )}

          <div className={styles.footer}>
            <Button type="submit" disabled={sending}>
              {sending ? <Spinner size={16} label={strings.composeSending} /> : strings.composeSend}
            </Button>
            <button type="button" className={styles.discard} onClick={onClose}>
              {strings.composeDiscard}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
