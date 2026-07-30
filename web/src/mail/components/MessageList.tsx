// The message list for the selected folder — one row per CONVERSATION (thread),
// Gmail-style: a compact single line (star · sender · subject — snippet · time),
// with the time swapped for archive / delete / read-toggle actions on hover.
// Unread threads read bold; the folder header carries a collapse toggle + search.
import { useEffect, useMemo, useState } from "react";
import {
  Archive,
  Mail,
  MailOpen,
  PanelLeftClose,
  PanelLeftOpen,
  Paperclip,
  Search,
  Star,
  Trash2,
} from "lucide-react";

import { strings } from "../../i18n";
import { IconButton, Spinner, cx } from "../../ds";
import { useJmapClient } from "../../jmap";
import type { EmailHeaders } from "../../jmap";
import type { Async } from "../state/useAsync";
import { formatDate, senderName, subjectOr } from "../format";
import { groupThreads, type ThreadRow } from "../threads";
import { DRAG_EMAIL_MIME } from "../dnd";
import styles from "./MessageList.module.css";

interface MessageListProps {
  folderName: string;
  emails: Async<EmailHeaders[]>;
  selectedThreadId: string | null;
  readIds: ReadonlySet<string>;
  flagOverrides: ReadonlyMap<string, boolean>;
  foldersCollapsed: boolean;
  onToggleFolders: () => void;
  onSelect: (thread: ThreadRow) => void;
  /** Per-row (hover) actions on a whole conversation. */
  onArchive: (thread: ThreadRow) => void;
  onDelete: (thread: ThreadRow) => void;
  onToggleRead: (thread: ThreadRow) => void;
  onToggleFlag: (thread: ThreadRow) => void;
}

export function MessageList({
  folderName,
  emails,
  selectedThreadId,
  readIds,
  flagOverrides,
  foldersCollapsed,
  onToggleFolders,
  onSelect,
  onArchive,
  onDelete,
  onToggleRead,
  onToggleFlag,
}: MessageListProps) {
  const client = useJmapClient();
  const [query, setQuery] = useState("");
  // Server-side full-text search across the account (`null` = folder view).
  const [results, setResults] = useState<EmailHeaders[] | null>(null);
  const isSearch = query.trim() !== "";

  useEffect(() => {
    const q = query.trim();
    if (q === "") {
      setResults(null);
      return undefined;
    }
    setResults(null); // show the spinner while the query runs
    let live = true;
    const timer = setTimeout(() => {
      client
        .searchEmails(q)
        .then((r) => {
          if (live) setResults(r);
        })
        .catch(() => {
          if (live) setResults([]);
        });
    }, 250); // debounce keystrokes
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [query, client]);

  const list = isSearch ? (results ?? []) : emails.status === "ready" ? (emails.data ?? []) : [];
  const threads = useMemo(
    () => groupThreads(list, readIds, flagOverrides),
    [list, readIds, flagOverrides],
  );
  const loading = isSearch ? results === null : emails.status === "loading";
  const error = !isSearch && emails.status === "error";

  return (
    <section className={styles.column}>
      <header className={styles.header}>
        <div className={styles.titleRow}>
          <IconButton
            size="sm"
            label={foldersCollapsed ? strings.expandFolders : strings.collapseFolders}
            icon={foldersCollapsed ? <PanelLeftOpen /> : <PanelLeftClose />}
            onClick={onToggleFolders}
          />
          <h1 className={styles.title}>{folderName}</h1>
        </div>
        <div className={styles.search}>
          <Search size={16} className={styles.searchIcon} />
          <input
            className={styles.searchInput}
            type="search"
            placeholder={strings.mailSearchPlaceholder}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            aria-label={strings.mailSearchPlaceholder}
          />
        </div>
      </header>

      {loading && (
        <div className={styles.state}>
          <Spinner size={22} />
          <p>{isSearch ? strings.mailSearching : strings.mailLoading}</p>
        </div>
      )}

      {error && (
        <div className={styles.state}>
          <p>{strings.mailListError}</p>
          <button type="button" className={styles.retry} onClick={emails.reload}>
            {strings.mailRetry}
          </button>
        </div>
      )}

      {!loading && !error && (
        <ul className={styles.list}>
          {threads.length === 0 && (
            <li className={styles.empty}>{isSearch ? strings.mailSearchEmpty : strings.mailEmpty}</li>
          )}
          {threads.map((thread) => {
            const email = thread.latest;
            const active = thread.threadId === selectedThreadId;
            return (
              <li
                key={thread.threadId}
                className={cx(
                  styles.row,
                  active && styles.active,
                  thread.hasUnread && styles.unread,
                )}
              >
                <button
                  type="button"
                  className={styles.flagBtn}
                  aria-label={thread.hasFlagged ? strings.unflag : strings.flag}
                  onClick={() => onToggleFlag(thread)}
                >
                  <Star className={cx(styles.star, thread.hasFlagged && styles.starOn)} />
                </button>
                <button
                  type="button"
                  className={styles.rowOpen}
                  onClick={() => onSelect(thread)}
                  aria-current={active ? "true" : undefined}
                  draggable
                  onDragStart={(e) => {
                    e.dataTransfer.setData(DRAG_EMAIL_MIME, thread.memberIds.join(","));
                    e.dataTransfer.effectAllowed = "move";
                  }}
                >
                  <span className={styles.sender}>
                    {senderName(email)}
                    {thread.count > 1 && <span className={styles.count}> ({thread.count})</span>}
                  </span>
                  <span className={styles.subjectWrap}>
                    <span className={styles.subject}>{subjectOr(email)}</span>
                    {email.preview.length > 0 && (
                      <span className={styles.snippet}> — {email.preview}</span>
                    )}
                  </span>
                  {thread.hasAttachment && <Paperclip className={styles.clip} aria-hidden="true" />}
                </button>
                <div className={styles.rowRight}>
                  <span className={styles.time}>{formatDate(email.receivedAt)}</span>
                  <div className={styles.actions}>
                    <button
                      type="button"
                      className={styles.actionBtn}
                      aria-label={strings.archive}
                      title={strings.archive}
                      onClick={() => onArchive(thread)}
                    >
                      <Archive size={16} />
                    </button>
                    <button
                      type="button"
                      className={styles.actionBtn}
                      aria-label={strings.delete}
                      title={strings.delete}
                      onClick={() => onDelete(thread)}
                    >
                      <Trash2 size={16} />
                    </button>
                    <button
                      type="button"
                      className={styles.actionBtn}
                      aria-label={thread.hasUnread ? strings.markRead : strings.markUnread}
                      title={thread.hasUnread ? strings.markRead : strings.markUnread}
                      onClick={() => onToggleRead(thread)}
                    >
                      {thread.hasUnread ? <MailOpen size={16} /> : <Mail size={16} />}
                    </button>
                  </div>
                </div>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
