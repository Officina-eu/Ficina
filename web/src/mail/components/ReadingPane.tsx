// The reading pane — a CONVERSATION view. It shows the whole thread (all its
// messages, across folders) stacked oldest-first, the newest expanded and the
// rest collapsed to a click-to-open summary. The action toolbar operates on the
// conversation: Reply/Forward act on the latest message; Flag toggles it;
// Archive/Delete/Move act on this folder's copies of the whole thread.
import { useEffect, useState } from "react";
import {
  Archive,
  FolderInput,
  Forward,
  MailOpen,
  MoreHorizontal,
  Reply,
  ReplyAll,
  Send,
  Sparkles,
  Star,
  Trash2,
} from "lucide-react";

import { strings } from "../../i18n";
import { IconButton, Menu, Spinner } from "../../ds";
import type { MenuItem } from "../../ds";
import { KEYWORD_FLAGGED, type EmailFull, type Mailbox, useJmapClient } from "../../jmap";
import { useAuth } from "../../auth";
import type { Async } from "../state/useAsync";
import { senderName, subjectOr } from "../format";
import { threadDigest } from "../body";
import { ThreadMessage } from "./ThreadMessage";
import styles from "./ReadingPane.module.css";

/** Below this many characters a thread isn't worth summarizing — the message is
 * already short enough to read directly, and a one-line "summary" adds noise. */
const SUMMARY_MIN_CHARS = 600;

type SummaryState =
  | { status: "off" }
  | { status: "loading" }
  | { status: "ready"; text: string };

const ROLE_ORDER: Record<string, number> = {
  inbox: 0,
  drafts: 1,
  sent: 2,
  archive: 3,
  junk: 4,
  trash: 5,
};

interface ReadingPaneProps {
  thread: Async<EmailFull[]>;
  mailboxes: Mailbox[];
  /** The folder currently being viewed (excluded from "Move to"). */
  currentMailboxId: string | null;
  flagOverrides: ReadonlyMap<string, boolean>;
  onReply: () => void;
  onReplyAll: () => void;
  onForward: () => void;
  /** Reply / reply-all / forward / delete one specific message in the thread. */
  onReplyMessage: (email: EmailFull) => void;
  onReplyAllMessage: (email: EmailFull) => void;
  onForwardMessage: (email: EmailFull) => void;
  onDeleteMessage: (email: EmailFull) => void;
  onToggleFlag: () => void;
  onArchive: () => void;
  onDelete: () => void;
  onMove: (targetMailboxId: string) => void;
  onMarkUnread: () => void;
}

export function ReadingPane({
  thread,
  mailboxes,
  currentMailboxId,
  flagOverrides,
  onReply,
  onReplyAll,
  onForward,
  onReplyMessage,
  onReplyAllMessage,
  onForwardMessage,
  onDeleteMessage,
  onToggleFlag,
  onArchive,
  onDelete,
  onMove,
  onMarkUnread,
}: ReadingPaneProps) {
  const { identity } = useAuth();
  const client = useJmapClient();
  const messages = thread.status === "ready" ? (thread.data ?? []) : [];
  const latest = messages.length > 0 ? messages[messages.length - 1] : undefined;

  // Expanded set: the newest message opens by default; reset when the thread
  // changes (keyed by the latest message id).
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set());
  useEffect(() => {
    setExpanded(latest !== undefined ? new Set([latest.id]) : new Set());
  }, [latest?.id]);

  // Whether the tenant has AI enabled — determines if we offer a summary.
  const [aiEnabled, setAiEnabled] = useState(false);
  useEffect(() => {
    let live = true;
    client
      .aiEnabled()
      .then((on) => live && setAiEnabled(on))
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [client]);

  // The Ficina conversation summary. Fetched when a long-enough thread opens and
  // AI is on; soft-degrades to nothing on any error (never blocks reading).
  const [summary, setSummary] = useState<SummaryState>({ status: "off" });
  useEffect(() => {
    if (!aiEnabled || latest === undefined) {
      setSummary({ status: "off" });
      return;
    }
    const digest = threadDigest(messages);
    if (digest.length < SUMMARY_MIN_CHARS) {
      setSummary({ status: "off" });
      return;
    }
    let live = true;
    setSummary({ status: "loading" });
    client
      .summarizeThread(digest)
      .then((text) => {
        if (live && text.trim().length > 0) setSummary({ status: "ready", text: text.trim() });
        else if (live) setSummary({ status: "off" });
      })
      .catch(() => live && setSummary({ status: "off" }));
    return () => {
      live = false;
    };
    // Re-run only when the conversation identity or the AI toggle changes; the
    // digest and client are derived from those and intentionally not deps.
  }, [aiEnabled, latest?.id]);

  if (thread.status === "loading") {
    return (
      <div className={styles.state}>
        <Spinner size={24} />
      </div>
    );
  }
  if (thread.status === "error") {
    return (
      <div className={styles.state}>
        <p>{strings.mailListError}</p>
        <button type="button" className={styles.retry} onClick={thread.reload}>
          {strings.mailRetry}
        </button>
      </div>
    );
  }
  if (latest === undefined) {
    return (
      <div className={styles.state}>
        <p>{strings.mailSelectPrompt}</p>
      </div>
    );
  }

  const flagged = flagOverrides.get(latest.id) ?? latest.keywords[KEYWORD_FLAGGED] === true;
  const me = identity?.email.toLowerCase();

  function toggle(id: string) {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  const moveItems: MenuItem[] = mailboxes
    .filter((m) => m.id !== currentMailboxId)
    .sort(
      (a, b) =>
        (ROLE_ORDER[a.role ?? ""] ?? 50) - (ROLE_ORDER[b.role ?? ""] ?? 50) ||
        a.name.localeCompare(b.name),
    )
    .map((m) => ({ key: m.id, label: m.name, onClick: () => onMove(m.id) }));

  const moreItems: MenuItem[] = [
    { key: "unread", label: strings.markUnread, icon: <MailOpen />, onClick: onMarkUnread },
    { key: "delete", label: strings.delete, icon: <Trash2 />, danger: true, onClick: onDelete },
  ];

  return (
    <article className={styles.pane}>
      <div className={styles.toolbar}>
        <button type="button" className={styles.replyBtn} onClick={onReply}>
          <Reply size={16} />
          <span>{strings.reply}</span>
        </button>
        <button type="button" className={styles.textBtn} onClick={onReplyAll}>
          <ReplyAll size={16} />
          <span>{strings.replyAll}</span>
        </button>
        <button type="button" className={styles.textBtn} onClick={onForward}>
          <Forward size={16} />
          <span>{strings.forward}</span>
        </button>
        <div className={styles.spacer} />
        <IconButton size="sm" label={strings.archive} icon={<Archive />} onClick={onArchive} />
        <Menu label={strings.moveTo} icon={<FolderInput />} items={moveItems} />
        <IconButton
          size="sm"
          label={flagged ? strings.unflag : strings.flag}
          active={flagged}
          icon={<Star className={flagged ? styles.starOn : ""} />}
          onClick={onToggleFlag}
        />
        <IconButton size="sm" label={strings.delete} icon={<Trash2 />} onClick={onDelete} />
        <Menu label={strings.moreActions} icon={<MoreHorizontal />} items={moreItems} />
      </div>

      <div className={styles.bodyScroll}>
        <div className={styles.subjectRow}>
          <h1 className={styles.subject}>{subjectOr(latest)}</h1>
          {messages.length > 1 && (
            <span className={styles.threadCount}>
              {messages.length} {strings.threadMessages}
            </span>
          )}
        </div>

        {summary.status !== "off" && (
          <section className={styles.summary} aria-live="polite">
            <div className={styles.summaryHead}>
              <Sparkles size={14} className={styles.summaryIcon} />
              <span>{strings.ficinaSummary}</span>
            </div>
            {summary.status === "loading" ? (
              <p className={styles.summaryPending}>{strings.summaryPending}</p>
            ) : (
              <p className={styles.summaryText}>{summary.text}</p>
            )}
          </section>
        )}

        <div className={styles.messages}>
          {messages.map((message) => (
            <ThreadMessage
              key={message.id}
              email={message}
              expanded={expanded.has(message.id)}
              me={me}
              onToggle={() => toggle(message.id)}
              onReply={() => onReplyMessage(message)}
              onReplyAll={() => onReplyAllMessage(message)}
              onForward={() => onForwardMessage(message)}
              onDelete={() => onDeleteMessage(message)}
            />
          ))}
        </div>
      </div>

      <div className={styles.quickReply}>
        <div className={styles.quickHead}>
          <Reply size={14} />
          <span className={styles.quickHeadLabel}>{strings.quickReplyTo(senderName(latest))}</span>
          <span className={styles.quickHeadHint}>{strings.quickReplyHint}</span>
        </div>
        <div className={styles.quickBar}>
          <div className={styles.replyField}>
            <button type="button" className={styles.quickInput} onClick={onReply}>
              {strings.replyToName(senderName(latest))}
            </button>
            <button type="button" className={styles.draftAi} onClick={onReply}>
              <Sparkles size={14} />
              <span>{strings.draftWithAi}</span>
            </button>
          </div>
          <button
            type="button"
            className={styles.send}
            onClick={onReply}
            aria-label={strings.reply}
          >
            <Send size={17} />
          </button>
        </div>
      </div>
    </article>
  );
}
