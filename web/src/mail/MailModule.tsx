// The Mail module: the four-column surface (folders · conversations · reading
// pane) and the state tying them together — folder + thread selection,
// optimistic read/flag state, and conversation-level actions (reply/forward on
// the latest message; flag, archive, delete, move, mark-unread, and
// drag-and-drop on the whole thread within the current folder).
import { useEffect, useState } from "react";
import type { CSSProperties } from "react";

import { strings } from "../i18n";
import { ResizeHandle, usePanelWidth } from "../ds";
import { KEYWORD_FLAGGED, useJmapClient } from "../jmap";
import type { EmailFull } from "../jmap";
import { useAuth } from "../auth";
import { useEmailHeaders, useMailboxes, useThread } from "./state/useMail";
import type { ThreadRow } from "./threads";
import { FolderSidebar } from "./components/FolderSidebar";
import { MessageList } from "./components/MessageList";
import { ReadingPane } from "./components/ReadingPane";
import { ComposeModal } from "./components/ComposeModal";
import type { ComposeContext } from "./components/ComposeModal";
import styles from "./MailModule.module.css";

export function MailModule() {
  const client = useJmapClient();
  const { identity } = useAuth();
  const mailboxes = useMailboxes();

  // Resizable panels (drag the dividers; persisted across sessions).
  const folders = usePanelWidth("ficina.mail.foldersWidth", 232, 176, 420);
  const list = usePanelWidth("ficina.mail.listWidth", 372, 300, 640);

  const [mailboxId, setMailboxId] = useState<string | null>(null);
  const [threadId, setThreadId] = useState<string | null>(null);
  const [readIds, setReadIds] = useState<ReadonlySet<string>>(new Set());
  const [flags, setFlags] = useState<ReadonlyMap<string, boolean>>(new Map());
  const [toast, setToast] = useState<string | null>(null);
  const [compose, setCompose] = useState<ComposeContext | null>(null);
  const [foldersCollapsed, setFoldersCollapsed] = useState<boolean>(() => {
    try {
      return localStorage.getItem("ficina.mail.foldersCollapsed") === "1";
    } catch {
      return false;
    }
  });

  function toggleFolders() {
    setFoldersCollapsed((collapsed) => {
      const next = !collapsed;
      try {
        localStorage.setItem("ficina.mail.foldersCollapsed", next ? "1" : "0");
      } catch {
        // ignore — collapse state simply won't persist
      }
      return next;
    });
  }

  const emails = useEmailHeaders(mailboxId);
  const thread = useThread(threadId);

  const boxes = mailboxes.status === "ready" ? (mailboxes.data ?? []) : [];
  const folderName = boxes.find((b) => b.id === mailboxId)?.name ?? strings.moduleMail;
  const draftsMailboxId =
    boxes.find((b) => b.role === "drafts")?.id ?? mailboxId ?? boxes[0]?.id ?? null;

  // The open conversation's messages, its latest, and the ids of the ones that
  // live in the current folder (actions apply to these).
  const threadMessages = thread.status === "ready" ? (thread.data ?? []) : [];
  const latest = threadMessages.length > 0 ? threadMessages[threadMessages.length - 1] : undefined;
  const currentFolderIds =
    mailboxId === null
      ? []
      : threadMessages.filter((m) => m.mailboxIds[mailboxId] === true).map((m) => m.id);

  // Default to the Inbox (or the first mailbox) once folders load.
  useEffect(() => {
    if (mailboxId !== null || mailboxes.status !== "ready") return;
    const list = mailboxes.data ?? [];
    const inbox = list.find((m) => m.role === "inbox") ?? list[0];
    if (inbox !== undefined) setMailboxId(inbox.id);
  }, [mailboxId, mailboxes.status, mailboxes.data]);

  // Toasts self-dismiss.
  useEffect(() => {
    if (toast === null) return undefined;
    const timer = setTimeout(() => setToast(null), 3500);
    return () => clearTimeout(timer);
  }, [toast]);

  const afterChange = (message: string) => {
    setToast(message);
    emails.reload();
    mailboxes.reload();
  };
  const fail = () => setToast(strings.mailActionFailed);

  function openMailbox(id: string) {
    setMailboxId(id);
    setThreadId(null);
  }

  function openThread(row: ThreadRow) {
    setThreadId(row.threadId);
    if (row.hasUnread) {
      setReadIds((prev) => {
        const next = new Set(prev);
        row.memberIds.forEach((id) => next.add(id));
        return next;
      });
      void client.setSeenMany(row.memberIds, true).catch(() => {
        // Optimistic; the server reconciles on the next folder load.
      });
    }
  }

  function toggleFlag(message: EmailFull) {
    const base = message.keywords[KEYWORD_FLAGGED] === true;
    const current = flags.get(message.id) ?? base;
    const next = !current;
    setFlags((prev) => new Map(prev).set(message.id, next));
    void client.setFlagged(message.id, next).catch(() => {
      setFlags((prev) => new Map(prev).set(message.id, current));
    });
  }

  // Move a set of messages (by id) from the current folder to another, used by
  // both the reading-pane "Move to" menu and drag-and-drop onto a folder.
  function moveIds(ids: string[], targetMailboxId: string) {
    if (mailboxId === null || targetMailboxId === mailboxId || ids.length === 0) return;
    if (ids.some((id) => currentFolderIds.includes(id))) setThreadId(null);
    void client
      .moveMany(ids, mailboxId, targetMailboxId)
      .then(() => afterChange(strings.mailMoved))
      .catch(fail);
  }

  function moveThread(targetMailboxId: string) {
    moveIds(currentFolderIds, targetMailboxId);
  }

  function archiveThread() {
    const archiveBox = boxes.find((b) => b.role === "archive");
    if (archiveBox === undefined || mailboxId === null || currentFolderIds.length === 0) {
      setToast(strings.archiveUnavailable);
      return;
    }
    moveIds(currentFolderIds, archiveBox.id);
  }

  // Delete the conversation: to Trash from a normal folder; permanently when
  // already in Trash (or when there is no Trash folder).
  function deleteThread() {
    if (currentFolderIds.length === 0) return;
    const trash = boxes.find((b) => b.role === "trash");
    const ids = currentFolderIds;
    setThreadId(null);
    if (trash === undefined || mailboxId === null || mailboxId === trash.id) {
      void client.destroyMany(ids).then(() => afterChange(strings.mailDeleted)).catch(fail);
    } else {
      void client.moveMany(ids, mailboxId, trash.id).then(() => afterChange(strings.mailDeleted)).catch(fail);
    }
  }

  function markThreadUnread() {
    if (currentFolderIds.length === 0) return;
    const ids = currentFolderIds;
    setReadIds((prev) => {
      const next = new Set(prev);
      ids.forEach((id) => next.delete(id));
      return next;
    });
    void client
      .setSeenMany(ids, false)
      .then(() => {
        emails.reload();
        mailboxes.reload();
      })
      .catch(fail);
  }

  function onSent() {
    setCompose(null);
    setToast(strings.composeSent);
    emails.reload();
    mailboxes.reload();
    if (threadId !== null) thread.reload();
  }

  const widthVars = {
    // Collapsed = a compact icon-only column (folders stay one-click reachable).
    "--sidebar-width": foldersCollapsed ? "56px" : `${folders.width}px`,
    "--list-width": `${list.width}px`,
  } as CSSProperties;

  return (
    <div className={styles.mail} style={widthVars}>
      <FolderSidebar
        mailboxes={mailboxes}
        selectedId={mailboxId}
        collapsed={foldersCollapsed}
        onSelect={openMailbox}
        onCompose={() => setCompose({ mode: "new" })}
        onDropMessage={moveIds}
      />
      {!foldersCollapsed && (
        <ResizeHandle
          ariaLabel={strings.resizeFolders}
          onResize={folders.applyDelta}
          onCommit={folders.commit}
          onReset={folders.reset}
        />
      )}
      <MessageList
        folderName={folderName}
        emails={emails}
        selectedThreadId={threadId}
        readIds={readIds}
        flagOverrides={flags}
        foldersCollapsed={foldersCollapsed}
        onToggleFolders={toggleFolders}
        onSelect={openThread}
      />
      <ResizeHandle
        ariaLabel={strings.resizeMessages}
        onResize={list.applyDelta}
        onCommit={list.commit}
        onReset={list.reset}
      />
      <ReadingPane
        thread={thread}
        mailboxes={boxes}
        currentMailboxId={mailboxId}
        flagOverrides={flags}
        onReply={() => latest !== undefined && setCompose({ mode: "reply", replyTo: latest })}
        onForward={() => latest !== undefined && setCompose({ mode: "forward", replyTo: latest })}
        onToggleFlag={() => latest !== undefined && toggleFlag(latest)}
        onArchive={archiveThread}
        onDelete={deleteThread}
        onMove={moveThread}
        onMarkUnread={markThreadUnread}
      />
      {compose !== null && (
        <ComposeModal
          context={compose}
          fromEmail={identity?.email ?? ""}
          fromName={identity?.name ?? ""}
          draftsMailboxId={draftsMailboxId}
          onClose={() => setCompose(null)}
          onSent={onSent}
        />
      )}
      {toast !== null && (
        <div className={styles.toast} role="status">
          {toast}
        </div>
      )}
    </div>
  );
}
