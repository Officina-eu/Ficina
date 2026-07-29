// The folders pane: the account's mailboxes with unread counts, roles first
// (Inbox at top). Selecting one drives the message list.
import {
  Archive,
  Folder,
  Inbox,
  Send,
  ShieldAlert,
  Trash2,
  FileText,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { strings } from "../../i18n";
import { Spinner } from "../../ds";
import type { Mailbox } from "../../jmap";
import type { Async } from "../state/useAsync";
import styles from "./MailboxList.module.css";

const ROLE_ICON: Record<string, LucideIcon> = {
  inbox: Inbox,
  sent: Send,
  drafts: FileText,
  trash: Trash2,
  archive: Archive,
  junk: ShieldAlert,
};

const ROLE_ORDER: Record<string, number> = {
  inbox: 0,
  drafts: 1,
  sent: 2,
  archive: 3,
  junk: 4,
  trash: 5,
};

function sortMailboxes(list: Mailbox[]): Mailbox[] {
  return [...list].sort((a, b) => {
    const ra = a.role !== null ? (ROLE_ORDER[a.role] ?? 50) : 100;
    const rb = b.role !== null ? (ROLE_ORDER[b.role] ?? 50) : 100;
    if (ra !== rb) return ra - rb;
    if (a.sortOrder !== b.sortOrder) return a.sortOrder - b.sortOrder;
    return a.name.localeCompare(b.name);
  });
}

interface MailboxListProps {
  mailboxes: Async<Mailbox[]>;
  selectedId: string | null;
  onSelect: (id: string) => void;
}

export function MailboxList({ mailboxes, selectedId, onSelect }: MailboxListProps) {
  return (
    <nav className={styles.list} aria-label={strings.mailFolders}>
      <h2 className={styles.heading}>{strings.mailFolders}</h2>

      {mailboxes.status === "loading" && (
        <div className={styles.state}>
          <Spinner size={18} />
        </div>
      )}

      {mailboxes.status === "error" && (
        <div className={styles.state}>
          <p>{strings.mailFolderError}</p>
          <button type="button" className={styles.retry} onClick={mailboxes.reload}>
            {strings.mailRetry}
          </button>
        </div>
      )}

      {mailboxes.status === "ready" &&
        sortMailboxes(mailboxes.data ?? []).map((box) => {
          const Icon = (box.role !== null ? ROLE_ICON[box.role] : undefined) ?? Folder;
          const active = box.id === selectedId;
          return (
            <button
              key={box.id}
              type="button"
              className={`${styles.item} ${active ? styles.active : ""}`}
              onClick={() => onSelect(box.id)}
              aria-current={active ? "true" : undefined}
            >
              <Icon className={styles.icon} strokeWidth={1.75} />
              <span className={styles.name}>{box.name}</span>
              {box.unreadEmails > 0 && <span className={styles.count}>{box.unreadEmails}</span>}
            </button>
          );
        })}
    </nav>
  );
}
