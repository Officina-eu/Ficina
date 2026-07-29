// The reading pane (Figma app shell): an action toolbar, the subject, folder/
// flag tags, the sender block, the message body, and a quick-reply bar. Body
// safety as before — plain text in Garamond, HTML isolated in a sandboxed,
// CSP-locked iframe. Compose/reply and the AI summary have no backend yet, so
// they are honest placeholders (the toolbar's real actions — flag, archive —
// work).
import {
  Archive,
  Forward,
  MoreHorizontal,
  Reply,
  ReplyAll,
  Send,
  Sparkles,
  Star,
} from "lucide-react";

import { strings } from "../../i18n";
import { Avatar, IconButton, Spinner } from "../../ds";
import { KEYWORD_FLAGGED, type EmailAddress, type EmailFull, type Mailbox } from "../../jmap";
import { useAuth } from "../../auth";
import type { Async } from "../state/useAsync";
import { formatDate, senderName, subjectOr } from "../format";
import { htmlContent, sandboxedHtml, textContent } from "../body";
import styles from "./ReadingPane.module.css";

interface ReadingPaneProps {
  email: Async<EmailFull | null>;
  mailboxes: Mailbox[];
  flagOverrides: ReadonlyMap<string, boolean>;
  onReply: () => void;
  onToggleFlag: (email: EmailFull) => void;
  onArchive: (email: EmailFull) => void;
}

export function ReadingPane({
  email,
  mailboxes,
  flagOverrides,
  onReply,
  onToggleFlag,
  onArchive,
}: ReadingPaneProps) {
  const { identity } = useAuth();

  if (email.status === "loading") {
    return (
      <div className={styles.state}>
        <Spinner size={24} />
      </div>
    );
  }
  if (email.status === "error") {
    return (
      <div className={styles.state}>
        <p>{strings.mailListError}</p>
        <button type="button" className={styles.retry} onClick={email.reload}>
          {strings.mailRetry}
        </button>
      </div>
    );
  }

  const message = email.data;
  if (message === null) {
    return (
      <div className={styles.state}>
        <p>{strings.mailSelectPrompt}</p>
      </div>
    );
  }

  const flagged = flagOverrides.get(message.id) ?? message.keywords[KEYWORD_FLAGGED] === true;
  const text = textContent(message);
  const html = text === null ? htmlContent(message) : null;
  const folderTags = mailboxes
    .filter((m) => m.role === null && message.mailboxIds[m.id] === true)
    .map((m) => m.name);

  function recipientLine(): string {
    const to = message?.to ?? null;
    if (to === null || to.length === 0) return "";
    const me = identity?.email.toLowerCase();
    if (me !== undefined && to.some((a) => a.email.toLowerCase() === me)) return "me";
    return to.map((a: EmailAddress) => (a.name !== null && a.name.length > 0 ? a.name : a.email)).join(", ");
  }

  return (
    <article className={styles.pane}>
      <div className={styles.toolbar}>
        <button type="button" className={styles.replyBtn} onClick={onReply}>
          <Reply size={16} />
          <span>{strings.reply}</span>
        </button>
        <button type="button" className={styles.textBtn} onClick={onReply}>
          <ReplyAll size={16} />
          <span>{strings.replyAll}</span>
        </button>
        <button type="button" className={styles.textBtn} onClick={onReply}>
          <Forward size={16} />
          <span>{strings.forward}</span>
        </button>
        <div className={styles.spacer} />
        <IconButton size="sm" label={strings.archive} icon={<Archive />} onClick={() => onArchive(message)} />
        <IconButton
          size="sm"
          label={flagged ? strings.unflag : strings.flag}
          active={flagged}
          icon={<Star className={flagged ? styles.starOn : ""} />}
          onClick={() => onToggleFlag(message)}
        />
        <IconButton size="sm" label={strings.moreActions} icon={<MoreHorizontal />} onClick={onReply} />
      </div>

      <div className={styles.bodyScroll}>
        <h1 className={styles.subject}>{subjectOr(message)}</h1>

        {(folderTags.length > 0 || flagged) && (
          <div className={styles.tags}>
            {folderTags.map((t) => (
              <span key={t} className={styles.tag}>
                {t}
              </span>
            ))}
            {flagged && <span className={`${styles.tag} ${styles.tagFlagged}`}>★ {strings.flag}</span>}
          </div>
        )}

        <div className={styles.sender}>
          <Avatar name={senderName(message)} email={message.from?.[0]?.email} size="md" />
          <div className={styles.senderText}>
            <div className={styles.senderTop}>
              <span className={styles.senderName}>{senderName(message)}</span>
              <span className={styles.date}>{formatDate(message.receivedAt)}</span>
            </div>
            <div className={styles.senderSub}>
              {message.from?.[0]?.email} · {strings.toLabel} {recipientLine()}
            </div>
          </div>
        </div>

        {text !== null && <pre className={styles.text}>{text}</pre>}
        {html !== null && (
          <iframe
            className={styles.html}
            title={subjectOr(message)}
            sandbox=""
            srcDoc={sandboxedHtml(html)}
          />
        )}
        {text === null && html === null && <p className={styles.empty}>{message.preview}</p>}

        <div className={styles.endOfMessage}>{strings.endOfMessage}</div>
      </div>

      <div className={styles.quickReply}>
        <div className={styles.quickHead}>
          <Reply size={14} />
          <span>
            {strings.replyTo} {senderName(message)}
          </span>
        </div>
        <div className={styles.quickBar}>
          <button type="button" className={styles.quickInput} onClick={onReply}>
            {strings.replyTo} {senderName(message)}…
          </button>
          <button type="button" className={styles.draftAi} onClick={onReply}>
            <Sparkles size={15} />
            <span>{strings.draftWithAi}</span>
          </button>
          <button type="button" className={styles.send} onClick={onReply} aria-label={strings.reply}>
            <Send size={17} />
          </button>
        </div>
      </div>
    </article>
  );
}
