// The folder sidebar (Figma app shell): the Compose action, the account's
// system folders with unread counts, and a FOLDERS section for custom
// mailboxes. Selecting a folder drives the message list.
import { useState } from "react";
import {
  Archive,
  Hash,
  Inbox,
  PenLine,
  Send,
  ShieldAlert,
  Trash2,
  FileText,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { strings } from "../../i18n";
import { Spinner, cx } from "../../ds";
import type { Mailbox } from "../../jmap";
import type { Async } from "../state/useAsync";
import { DRAG_EMAIL_MIME } from "../dnd";
import styles from "./FolderSidebar.module.css";

const ROLE_ICON: Record<string, LucideIcon> = {
  inbox: Inbox,
  drafts: FileText,
  sent: Send,
  archive: Archive,
  junk: ShieldAlert,
  trash: Trash2,
};

const ROLE_ORDER: Record<string, number> = {
  inbox: 0,
  drafts: 1,
  sent: 2,
  archive: 3,
  junk: 4,
  trash: 5,
};

function ordered(list: Mailbox[]): { system: Mailbox[]; custom: Mailbox[] } {
  const system = list
    .filter((m) => m.role !== null)
    .sort((a, b) => (ROLE_ORDER[a.role ?? ""] ?? 50) - (ROLE_ORDER[b.role ?? ""] ?? 50));
  const custom = list
    .filter((m) => m.role === null)
    .sort((a, b) => a.sortOrder - b.sortOrder || a.name.localeCompare(b.name));
  return { system, custom };
}

interface FolderSidebarProps {
  mailboxes: Async<Mailbox[]>;
  selectedId: string | null;
  /** When collapsed the panel is a compact icon-only column. */
  collapsed: boolean;
  onSelect: (id: string) => void;
  onCompose: () => void;
  /** Drop dragged messages (a whole conversation) into a folder — moves them. */
  onDropMessage: (emailIds: string[], mailboxId: string) => void;
}

export function FolderSidebar({
  mailboxes,
  selectedId,
  collapsed,
  onSelect,
  onCompose,
  onDropMessage,
}: FolderSidebarProps) {
  const [dragOverId, setDragOverId] = useState<string | null>(null);
  function row(box: Mailbox, Icon: LucideIcon) {
    const active = box.id === selectedId;
    return (
      <button
        key={box.id}
        type="button"
        className={cx(styles.item, active && styles.active, dragOverId === box.id && styles.dropTarget)}
        onClick={() => onSelect(box.id)}
        aria-current={active ? "true" : undefined}
        title={box.name}
        onDragOver={(e) => {
          if (e.dataTransfer.types.includes(DRAG_EMAIL_MIME)) {
            e.preventDefault();
            e.dataTransfer.dropEffect = "move";
          }
        }}
        onDragEnter={() => setDragOverId(box.id)}
        onDragLeave={(e) => {
          if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setDragOverId(null);
        }}
        onDrop={(e) => {
          e.preventDefault();
          const ids = e.dataTransfer.getData(DRAG_EMAIL_MIME).split(",").filter((s) => s !== "");
          setDragOverId(null);
          if (ids.length > 0) onDropMessage(ids, box.id);
        }}
      >
        <Icon className={styles.icon} strokeWidth={1.75} />
        <span className={styles.name}>{box.name}</span>
        {box.unreadEmails > 0 && <span className={styles.count}>{box.unreadEmails}</span>}
      </button>
    );
  }

  const { system, custom } = ordered(mailboxes.data ?? []);

  return (
    <nav className={cx(styles.sidebar, collapsed && styles.collapsed)} aria-label={strings.mailFolders}>
      <button type="button" className={styles.compose} onClick={onCompose} title={strings.compose}>
        <PenLine size={17} strokeWidth={2} />
        <span className={styles.composeLabel}>{strings.compose}</span>
      </button>

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

      {mailboxes.status === "ready" && (
        <div className={styles.scroll}>
          <div className={styles.group}>
            {system.map((box) => row(box, (box.role !== null ? ROLE_ICON[box.role] : undefined) ?? Hash))}
          </div>
          {custom.length > 0 && (
            <div className={styles.group}>
              <h2 className={styles.heading}>{strings.mailFolders}</h2>
              {custom.map((box) => row(box, Hash))}
            </div>
          )}
        </div>
      )}
    </nav>
  );
}
