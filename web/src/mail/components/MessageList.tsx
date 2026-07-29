// The message list for the selected folder — one row per CONVERSATION (thread).
// A header with the folder name + a collapse toggle + a search box, then rows
// showing the latest message's sender/subject/preview/time, an unread dot, a
// flag star, and a message-count badge when the thread has more than one.
import { useMemo, useState } from "react";
import { PanelLeftClose, PanelLeftOpen, Paperclip, Search, Star } from "lucide-react";

import { strings } from "../../i18n";
import { Avatar, IconButton, Spinner } from "../../ds";
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

function matches(row: ThreadRow, q: string): boolean {
  const e = row.latest;
  return `${senderName(e)} ${e.subject ?? ""} ${e.preview}`.toLowerCase().includes(q);
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
  const [query, setQuery] = useState("");
  const list = emails.status === "ready" ? (emails.data ?? []) : [];
  const threads = useMemo(
    () => groupThreads(list, readIds, flagOverrides),
    [list, readIds, flagOverrides],
  );
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q === "" ? threads : threads.filter((t) => matches(t, q));
  }, [threads, query]);

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

      {emails.status === "loading" && (
        <div className={styles.state}>
          <Spinner size={22} />
          <p>{strings.mailLoading}</p>
        </div>
      )}

      {emails.status === "error" && (
        <div className={styles.state}>
          <p>{strings.mailListError}</p>
          <button type="button" className={styles.retry} onClick={emails.reload}>
            {strings.mailRetry}
          </button>
        </div>
      )}

      {emails.status === "ready" && (
        <ul className={styles.list}>
          {filtered.length === 0 && (
            <li className={styles.empty}>{query === "" ? strings.mailEmpty : strings.mailSearchEmpty}</li>
          )}
          {filtered.map((thread) => {
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
