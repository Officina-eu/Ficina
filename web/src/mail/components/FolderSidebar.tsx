// The folder sidebar (Figma app shell): the Compose action, the account's
// system folders with unread counts, and a FOLDERS section for custom
// mailboxes — create, rename (inline), nest (parent/child), color, and delete.
// Selecting a folder drives the message list.
import { useState } from "react";
import type { KeyboardEvent, ReactNode } from "react";
import {
  Archive,
  CalendarClock,
  Clock,
  FolderPlus,
  Hash,
  Inbox,
  MoreHorizontal,
  PenLine,
  Pencil,
  Plus,
  Send,
  ShieldAlert,
  Star,
  Trash2,
  FileText,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { strings } from "../../i18n";
import { Spinner, cx } from "../../ds";
import type { Mailbox, SharedMailbox } from "../../jmap";
import type { Async } from "../state/useAsync";
import { DRAG_EMAIL_MIME } from "../dnd";
import styles from "./FolderSidebar.module.css";

const ROLE_ICON: Record<string, LucideIcon> = {
  inbox: Inbox,
  snoozed: Clock,
  drafts: FileText,
  scheduled: CalendarClock,
  sent: Send,
  archive: Archive,
  junk: ShieldAlert,
  trash: Trash2,
};

const ROLE_ORDER: Record<string, number> = {
  inbox: 0,
  snoozed: 1,
  drafts: 2,
  scheduled: 3,
  sent: 4,
  archive: 5,
  junk: 6,
  trash: 7,
};

function systemFolders(list: Mailbox[]): Mailbox[] {
  return list
    .filter((m) => m.role !== null)
    .sort((a, b) => (ROLE_ORDER[a.role ?? ""] ?? 50) - (ROLE_ORDER[b.role ?? ""] ?? 50));
}

/** Custom folders in tree order (parent before children), each with its depth. */
function nestCustom(list: Mailbox[]): { box: Mailbox; depth: number }[] {
  const custom = list.filter((m) => m.role === null);
  const ids = new Set(custom.map((m) => m.id));
  const byParent = new Map<string | null, Mailbox[]>();
  for (const m of custom) {
    const p = m.parentId !== null && ids.has(m.parentId) ? m.parentId : null;
    const arr = byParent.get(p) ?? [];
    arr.push(m);
    byParent.set(p, arr);
  }
  for (const arr of byParent.values()) {
    arr.sort((a, b) => a.sortOrder - b.sortOrder || a.name.localeCompare(b.name));
  }
  const out: { box: Mailbox; depth: number }[] = [];
  const walk = (parent: string | null, depth: number) => {
    for (const m of byParent.get(parent) ?? []) {
      out.push({ box: m, depth });
      walk(m.id, depth + 1);
    }
  };
  walk(null, 0);
  return out;
}

/** The label color palette (warm-workshop hues + a few universals). */
const LABEL_COLORS = [
  "#5b8a72", "#3f7cac", "#7b6cae", "#c07a3e",
  "#c0603e", "#b03a4b", "#4c9a8f", "#8a8f3a",
];

interface FolderSidebarProps {
  mailboxes: Async<Mailbox[]>;
  selectedId: string | null;
  collapsed: boolean;
  /** Shared mailboxes the user was delegated (ADR 0017). */
  shared: SharedMailbox[];
  /** The open account: a shared mailbox id, or null for the user's own. */
  activeAccount: string | null;
  /** Label for the user's own mailbox in the switcher. */
  ownLabel: string;
  onSwitchAccount: (id: string | null) => void;
  onSelect: (id: string) => void;
  onCompose: () => void;
  onDropMessage: (emailIds: string[], mailboxId: string) => void;
  onSetColor: (mailboxId: string, color: string | null) => void;
  /** Create a folder (optionally nested under `parentId`). */
  onCreateFolder: (name: string, parentId: string | null) => void;
  onRenameFolder: (id: string, name: string) => void;
  onDeleteFolder: (box: Mailbox) => void;
  /** Whether the cross-folder Flagged smart view is the active selection. */
  flaggedActive: boolean;
  onSelectFlagged: () => void;
  /** Rendered below the folder list (the Categories section). Hidden when the
   * sidebar is collapsed, alongside the other labels. */
  extraSection?: ReactNode;
}

export function FolderSidebar({
  mailboxes,
  selectedId,
  collapsed,
  shared,
  activeAccount,
  ownLabel,
  onSwitchAccount,
  onSelect,
  onCompose,
  onDropMessage,
  onSetColor,
  onCreateFolder,
  onRenameFolder,
  onDeleteFolder,
  flaggedActive,
  onSelectFlagged,
  extraSection,
}: FolderSidebarProps) {
  const [dragOverId, setDragOverId] = useState<string | null>(null);
  const [menu, setMenu] = useState<{ box: Mailbox; x: number; y: number } | null>(null);
  const [editing, setEditing] = useState<{ id: string; value: string } | null>(null);
  // A pending new folder, with the parent it nests under (null = root).
  const [creating, setCreating] = useState<{ parentId: string | null; value: string } | null>(null);

  function commitRename() {
    if (editing !== null && editing.value.trim().length > 0) {
      onRenameFolder(editing.id, editing.value.trim());
    }
    setEditing(null);
  }
  function commitCreate() {
    if (creating !== null && creating.value.trim().length > 0) {
      onCreateFolder(creating.value.trim(), creating.parentId);
    }
    setCreating(null);
  }
  function onEditKey(e: KeyboardEvent<HTMLInputElement>, commit: () => void, cancel: () => void) {
    if (e.key === "Enter") commit();
    else if (e.key === "Escape") cancel();
  }

  function row(box: Mailbox, leading: ReactNode, opts?: { colorable?: boolean; depth?: number }) {
    const active = box.id === selectedId;
    const depth = opts?.depth ?? 0;
    if (editing?.id === box.id) {
      return (
        <div key={box.id} className={styles.item} style={{ paddingLeft: 12 + depth * 14 }}>
          {leading}
          <input
            className={styles.rename}
            value={editing.value}
            autoFocus
            onChange={(e) => setEditing({ id: box.id, value: e.target.value })}
            onBlur={commitRename}
            onKeyDown={(e) => onEditKey(e, commitRename, () => setEditing(null))}
            aria-label={strings.folderRename}
          />
        </div>
      );
    }
    return (
      <button
        key={box.id}
        type="button"
        className={cx(styles.item, active && styles.active, dragOverId === box.id && styles.dropTarget)}
        style={depth > 0 ? { paddingLeft: 12 + depth * 14 } : undefined}
        onClick={() => onSelect(box.id)}
        aria-current={active ? "true" : undefined}
        title={box.name}
        onContextMenu={
          opts?.colorable
            ? (e) => {
                e.preventDefault();
                setMenu({ box, x: e.clientX, y: e.clientY });
              }
            : undefined
        }
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
        {leading}
        <span className={styles.name}>{box.name}</span>
        {box.unreadEmails > 0 && <span className={styles.count}>{box.unreadEmails}</span>}
      </button>
    );
  }

  function labelDot(box: Mailbox): ReactNode {
    return (
      <span
        className={styles.dot}
        style={box.color !== null ? { background: box.color } : undefined}
        aria-hidden
      />
    );
  }

  function newFolderInput(parentId: string | null, depth: number) {
    return (
      <div className={styles.item} style={{ paddingLeft: 12 + depth * 14 }}>
        <span className={styles.dot} aria-hidden />
        <input
          className={styles.rename}
          value={creating?.value ?? ""}
          autoFocus
          placeholder={strings.folderNamePlaceholder}
          onChange={(e) => setCreating({ parentId, value: e.target.value })}
          onBlur={commitCreate}
          onKeyDown={(e) => onEditKey(e, commitCreate, () => setCreating(null))}
          aria-label={strings.folderNew}
        />
      </div>
    );
  }

  const system = systemFolders(mailboxes.data ?? []);
  const custom = nestCustom(mailboxes.data ?? []);

  return (
    <nav className={cx(styles.sidebar, collapsed && styles.collapsed)} aria-label={strings.mailFolders}>
      {!collapsed && shared.length > 0 && (
        <label className={styles.switcher}>
          <span className={styles.switcherLabel}>{strings.sharedMailboxLabel}</span>
          <select
            className={styles.switcherSelect}
            value={activeAccount ?? ""}
            onChange={(e) => onSwitchAccount(e.target.value === "" ? null : e.target.value)}
            aria-label={strings.sharedMailboxLabel}
          >
            <option value="">{ownLabel}</option>
            {shared.map((s) => (
              <option key={s.id} value={s.id}>
                {s.name}
                {s.readOnly ? ` (${strings.sharedReadOnly})` : ""}
              </option>
            ))}
          </select>
        </label>
      )}
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
            {system.map((box) => {
              const Icon = (box.role !== null ? ROLE_ICON[box.role] : undefined) ?? Hash;
              return row(box, <Icon className={styles.icon} strokeWidth={1.75} />);
            })}
            {/* The cross-folder Flagged smart view — a virtual folder, so it
                sits with the system folders but drives its own selection. */}
            <button
              type="button"
              className={cx(styles.item, flaggedActive && styles.active)}
              onClick={onSelectFlagged}
              aria-current={flaggedActive ? "true" : undefined}
              title={strings.flaggedView}
            >
              <Star className={styles.icon} strokeWidth={1.75} />
              <span className={styles.name}>{strings.flaggedView}</span>
            </button>
          </div>
          <div className={styles.group}>
            <div className={styles.groupHead}>
              <h2 className={styles.heading}>{strings.mailFolders}</h2>
              <button
                type="button"
                className={styles.newFolder}
                onClick={() => setCreating({ parentId: null, value: "" })}
                title={strings.folderNew}
                aria-label={strings.folderNew}
              >
                <FolderPlus size={15} />
              </button>
            </div>
            {custom.map(({ box, depth }) => (
              <div key={box.id} className={styles.rowWrap}>
                {row(box, labelDot(box), { colorable: true, depth })}
                {editing?.id !== box.id && (
                  <button
                    type="button"
                    className={styles.kebab}
                    aria-label={strings.folderActions(box.name)}
                    title={strings.folderActions(box.name)}
                    onClick={(e) => {
                      e.stopPropagation();
                      const r = e.currentTarget.getBoundingClientRect();
                      setMenu({ box, x: r.right, y: r.bottom });
                    }}
                  >
                    <MoreHorizontal size={15} />
                  </button>
                )}
                {creating?.parentId === box.id && newFolderInput(box.id, depth + 1)}
              </div>
            ))}
            {creating?.parentId === null && newFolderInput(null, 0)}
          </div>
          {!collapsed && extraSection}
        </div>
      )}

      {menu !== null && (
        <>
          <button
            type="button"
            className={styles.pickerScrim}
            aria-hidden
            tabIndex={-1}
            onClick={() => setMenu(null)}
            onContextMenu={(e) => {
              e.preventDefault();
              setMenu(null);
            }}
          />
          <div
            className={styles.palette}
            role="menu"
            aria-label={menu.box.name}
            style={{ left: Math.min(menu.x, window.innerWidth - 200), top: menu.y }}
          >
            <button
              type="button"
              className={styles.menuItem}
              onClick={() => {
                setCreating({ parentId: menu.box.id, value: "" });
                setMenu(null);
              }}
            >
              <Plus size={14} />
              {strings.folderNewSub}
            </button>
            <button
              type="button"
              className={styles.menuItem}
              onClick={() => {
                setEditing({ id: menu.box.id, value: menu.box.name });
                setMenu(null);
              }}
            >
              <Pencil size={14} />
              {strings.folderRename}
            </button>
            <div className={styles.menuDivider} />
            <span className={styles.paletteHead}>{strings.labelColor}</span>
            <div className={styles.swatches}>
              {LABEL_COLORS.map((c) => (
                <button
                  key={c}
                  type="button"
                  className={styles.swatch}
                  style={{ background: c }}
                  aria-label={c}
                  onClick={() => {
                    onSetColor(menu.box.id, c);
                    setMenu(null);
                  }}
                />
              ))}
            </div>
            <button
              type="button"
              className={styles.clearColor}
              onClick={() => {
                onSetColor(menu.box.id, null);
                setMenu(null);
              }}
            >
              {strings.labelColorClear}
            </button>
            <div className={styles.menuDivider} />
            <button
              type="button"
              className={cx(styles.menuItem, styles.menuDanger)}
              onClick={() => {
                onDeleteFolder(menu.box);
                setMenu(null);
              }}
            >
              <Trash2 size={14} />
              {strings.folderDelete}
            </button>
          </div>
        </>
      )}
    </nav>
  );
}
