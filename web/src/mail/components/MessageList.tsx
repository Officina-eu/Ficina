// The message list for the selected folder — one row per CONVERSATION (thread).
// A header with the folder name + a collapse toggle + a search box, then rows
// showing the latest message's sender/subject/preview/time, an unread dot, a
// flag star, and a message-count badge when the thread has more than one.
import { useEffect, useMemo, useState } from "react";
import { PanelLeftClose, PanelLeftOpen, Paperclip, Search, Star } from "lucide-react";

import { strings } from "../../i18n";
import { Avatar, IconButton, Spinner } from "../../ds";
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
              <li key={thread.threadId}>
                <button
                  type="button"
                  className={`${styles.row} ${active ? styles.active : ""} ${thread.hasUnread ? styles.unread : ""}`}
                  onClick={() => onSelect(thread)}
                  aria-current={active ? "true" : undefined}
                  draggable
                  onDragStart={(e) => {
                    e.dataTransfer.setData(DRAG_EMAIL_MIME, thread.memberIds.join(","));
                    e.dataTransfer.effectAllowed = "move";
                  }}
                >
                  <Avatar name={senderName(email)} email={email.from?.[0]?.email} size="md" />
                  <span className={styles.body}>
                    <span className={styles.topline}>
                      {thread.hasUnread && <span className={styles.dot} aria-hidden="true" />}
                      <span className={styles.sender}>{senderName(email)}</span>
                      {thread.count > 1 && <span className={styles.threadCount}>{thread.count}</span>}
                      {thread.hasFlagged && <Star className={styles.star} aria-label={strings.flag} />}
                      <span className={styles.time}>{formatDate(email.receivedAt)}</span>
                    </span>
                    <span className={styles.subject}>
                      {subjectOr(email)}
                      {thread.hasAttachment && <Paperclip className={styles.clip} aria-hidden="true" />}
                    </span>
                    <span className={styles.preview}>{email.preview}</span>
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
