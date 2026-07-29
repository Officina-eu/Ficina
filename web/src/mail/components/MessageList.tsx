// The message list for the selected folder (Figma app shell): a header with
// the folder name and a search box, then rows with the sender's avatar,
// name, subject, preview, time, an unread dot, and a flag star. Search filters
// the loaded rows instantly by sender, subject, or preview.
import { useMemo, useState } from "react";
import { Paperclip, Search, Star } from "lucide-react";

import { strings } from "../../i18n";
import { Avatar, Spinner } from "../../ds";
import { KEYWORD_FLAGGED, type EmailHeaders } from "../../jmap";
import type { Async } from "../state/useAsync";
import { formatDate, isUnread, senderName, subjectOr } from "../format";
import styles from "./MessageList.module.css";

interface MessageListProps {
  folderName: string;
  emails: Async<EmailHeaders[]>;
  selectedId: string | null;
  readIds: ReadonlySet<string>;
  flagOverrides: ReadonlyMap<string, boolean>;
  onSelect: (email: EmailHeaders) => void;
}

function matches(email: EmailHeaders, q: string): boolean {
  const hay = `${senderName(email)} ${email.subject ?? ""} ${email.preview}`.toLowerCase();
  return hay.includes(q);
}

export function MessageList({
  folderName,
  emails,
  selectedId,
  readIds,
  flagOverrides,
  onSelect,
}: MessageListProps) {
  const [query, setQuery] = useState("");
  const list = emails.status === "ready" ? (emails.data ?? []) : [];
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q === "" ? list : list.filter((e) => matches(e, q));
  }, [list, query]);

  return (
    <section className={styles.column}>
      <header className={styles.header}>
        <h1 className={styles.title}>{folderName}</h1>
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
          {filtered.map((email) => {
            const unread = isUnread(email) && !readIds.has(email.id);
            const active = email.id === selectedId;
            const flagged = flagOverrides.get(email.id) ?? email.keywords[KEYWORD_FLAGGED] === true;
            return (
              <li key={email.id}>
                <button
                  type="button"
                  className={`${styles.row} ${active ? styles.active : ""} ${unread ? styles.unread : ""}`}
                  onClick={() => onSelect(email)}
                  aria-current={active ? "true" : undefined}
                >
                  <Avatar name={senderName(email)} email={email.from?.[0]?.email} size="md" />
                  <span className={styles.body}>
                    <span className={styles.topline}>
                      {unread && <span className={styles.dot} aria-hidden="true" />}
                      <span className={styles.sender}>{senderName(email)}</span>
                      {flagged && <Star className={styles.star} aria-label={strings.flag} />}
                      <span className={styles.time}>{formatDate(email.receivedAt)}</span>
                    </span>
                    <span className={styles.subject}>
                      {subjectOr(email)}
                      {email.hasAttachment && <Paperclip className={styles.clip} aria-hidden="true" />}
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
