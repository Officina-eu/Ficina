// The message list for the selected folder: sender, subject, preview, time,
// and an unread dot. Selecting a row opens it in the reading pane.
import { Paperclip } from "lucide-react";

import { strings } from "../../i18n";
import { Spinner } from "../../ds";
import type { EmailHeaders } from "../../jmap";
import type { Async } from "../state/useAsync";
import { formatDate, isUnread, senderName, subjectOr } from "../format";
import styles from "./MessageList.module.css";

interface MessageListProps {
  emails: Async<EmailHeaders[]>;
  selectedId: string | null;
  /** Ids marked read this session (optimistic — before the next folder load). */
  readIds: ReadonlySet<string>;
  onSelect: (email: EmailHeaders) => void;
}

export function MessageList({ emails, selectedId, readIds, onSelect }: MessageListProps) {
  if (emails.status === "loading") {
    return (
      <div className={styles.state}>
        <Spinner size={22} />
        <p>{strings.mailLoading}</p>
      </div>
    );
  }

  if (emails.status === "error") {
    return (
      <div className={styles.state}>
        <p>{strings.mailListError}</p>
        <button type="button" className={styles.retry} onClick={emails.reload}>
          {strings.mailRetry}
        </button>
      </div>
    );
  }

  const list = emails.data ?? [];
  if (list.length === 0) {
    return (
      <div className={styles.state}>
        <p>{strings.mailEmpty}</p>
      </div>
    );
  }

  return (
    <ul className={styles.list}>
      {list.map((email) => {
        const unread = isUnread(email) && !readIds.has(email.id);
        const active = email.id === selectedId;
        return (
          <li key={email.id}>
            <button
              type="button"
              className={`${styles.row} ${active ? styles.active : ""} ${unread ? styles.unread : ""}`}
              onClick={() => onSelect(email)}
              aria-current={active ? "true" : undefined}
            >
              <span className={styles.dot} aria-hidden="true" />
              <span className={styles.body}>
                <span className={styles.topline}>
                  <span className={styles.sender}>{senderName(email)}</span>
                  <span className={styles.time}>{formatDate(email.receivedAt)}</span>
                </span>
                <span className={styles.subject}>
                  {subjectOr(email)}
                  {email.hasAttachment && (
                    <Paperclip className={styles.clip} aria-label="Has attachment" />
                  )}
                </span>
                <span className={styles.preview}>{email.preview}</span>
              </span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}
