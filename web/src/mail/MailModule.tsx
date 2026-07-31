// The Mail module: the four-column surface (folders · conversations · reading
// pane) and the state tying them together — folder + thread selection,
// optimistic read/flag state, and conversation-level actions (reply/forward on
// the latest message; flag, archive, delete, move, mark-unread, and
// drag-and-drop on the whole thread within the current folder).
import { useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";

import { strings } from "../i18n";
import { ResizeHandle, cx, usePanelWidth } from "../ds";
import { KEYWORD_FLAGGED, useJmapClient } from "../jmap";
import type { EmailFull } from "../jmap";
import { useAuth } from "../auth";
import { useEmailHeaders, useMailboxes, useThread } from "./state/useMail";
import type { ThreadRow } from "./threads";
import { FolderSidebar } from "./components/FolderSidebar";
import { MessageList } from "./components/MessageList";
import { ReadingPane } from "./components/ReadingPane";
import { ComposeModal, formatSendAt } from "./components/ComposeModal";
import type { ComposeContext, QueuedSend } from "./components/ComposeModal";
import styles from "./MailModule.module.css";

export function MailModule() {
  const client = useJmapClient();
  const { identity } = useAuth();
  const mailboxes = useMailboxes();

  // Resizable panels (drag the dividers; persisted across sessions).
  const folders = usePanelWidth("alo.mail.foldersWidth", 232, 176, 420);
  const list = usePanelWidth("alo.mail.listWidth", 372, 300, 640);

  const [mailboxId, setMailboxId] = useState<string | null>(null);
  const [threadId, setThreadId] = useState<string | null>(null);
  const [readIds, setReadIds] = useState<ReadonlySet<string>>(new Set());
  const [flags, setFlags] = useState<ReadonlyMap<string, boolean>>(new Map());
  const [toast, setToast] = useState<string | null>(null);
  const [compose, setCompose] = useState<ComposeContext | null>(null);
  // The user's signature + tenant footer, inserted into new/reply drafts.
  const [mailSettings, setMailSettings] = useState<{ signature: string; orgFooter: string }>({
    signature: "",
    orgFooter: "",
  });
  const [foldersCollapsed, setFoldersCollapsed] = useState<boolean>(() => {
    try {
      return localStorage.getItem("alo.mail.foldersCollapsed") === "1";
    } catch {
      return false;
    }
  });

  function toggleFolders() {
    setFoldersCollapsed((collapsed) => {
      const next = !collapsed;
      try {
        localStorage.setItem("alo.mail.foldersCollapsed", next ? "1" : "0");
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

  // Load the signature + org footer once, for the compose surface.
  useEffect(() => {
    let live = true;
    void client
      .mailSettings()
      .then((s) => {
        if (live) setMailSettings(s);
      })
      .catch(() => {
        // best-effort — compose just opens without a signature
      });
    return () => {
      live = false;
    };
  }, [client]);

  const afterChange = (message: string) => {
    setToast(message);
    emails.reload();
    mailboxes.reload();
  };
  const fail = () => setToast(strings.mailActionFailed);

  // Undo send: a created draft is held for a few seconds before it is actually
  // submitted, so a mistaken send can be taken back — Undo just leaves it in
  // Drafts. One send is held at a time.
  const [pendingSend, setPendingSend] = useState<QueuedSend | null>(null);
  const pendingRef = useRef<QueuedSend | null>(null);
  const undoTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  async function flushSend() {
    const queued = pendingRef.current;
    if (queued === null) return;
    pendingRef.current = null;
    setPendingSend(null);
    if (undoTimer.current !== null) {
      clearTimeout(undoTimer.current);
      undoTimer.current = null;
    }
    try {
      await client.submitEmail(queued.emailId, queued.fromEmail, queued.rcpts);
      afterChange(strings.composeSent);
      if (threadId !== null) thread.reload(); // a sent reply joins the open thread
    } catch {
      setToast(strings.composeSendError);
    }
  }

  function queueSend(queued: QueuedSend) {
    if (pendingRef.current !== null) void flushSend(); // never hold two at once
    setCompose(null);
    pendingRef.current = queued;
    setPendingSend(queued);
    undoTimer.current = setTimeout(() => void flushSend(), 5000);
  }

  function undoSend() {
    if (undoTimer.current !== null) {
      clearTimeout(undoTimer.current);
      undoTimer.current = null;
    }
    pendingRef.current = null;
    setPendingSend(null);
    setToast(strings.composeSendUndone);
    emails.reload();
    mailboxes.reload();
  }

  // Don't silently drop a queued send if the module unmounts mid-window.
  const flushRef = useRef(flushSend);
  flushRef.current = flushSend;
  useEffect(() => () => void flushRef.current(), []);

  // Send later: the draft is created; schedule it server-side (it moves to the
  // Scheduled mailbox and a sweeper sends it when due). No Undo window — the
  // Scheduled folder's "Cancel send" is the take-back.
  async function scheduleSend(queued: QueuedSend & { sendAt: number }) {
    setCompose(null);
    try {
      await client.scheduleSend(queued.emailId, queued.fromEmail, queued.rcpts, queued.sendAt);
      afterChange(strings.mailScheduled(formatSendAt(queued.sendAt)));
    } catch {
      setToast(strings.scheduleError);
      emails.reload();
      mailboxes.reload();
    }
  }

  // Set (or clear) a label's color, then refresh the folder list.
  async function setLabelColor(id: string, color: string | null) {
    try {
      await client.setMailboxColor(id, color);
      mailboxes.reload();
    } catch {
      fail();
    }
  }

  // Folder management: create (optionally nested), rename, delete.
  async function createFolder(name: string, parentId: string | null) {
    try {
      await client.createMailbox(name, parentId);
      mailboxes.reload();
    } catch {
      setToast(strings.folderActionFailed);
    }
  }
  async function renameFolder(id: string, name: string) {
    try {
      await client.renameMailbox(id, name);
      mailboxes.reload();
    } catch {
      setToast(strings.folderActionFailed);
    }
  }
  async function deleteFolder(box: { id: string; name: string }) {
    if (!window.confirm(strings.folderDeleteConfirm(box.name))) return;
    try {
      await client.deleteMailbox(box.id);
      if (mailboxId === box.id) setMailboxId(null);
      mailboxes.reload();
    } catch {
      setToast(strings.folderActionFailed);
    }
  }

  // Block a sender: append a server-side rule that files their mail to Junk.
  async function blockSender(email: string) {
    try {
      await client.blockSender(email);
      setToast(strings.senderBlocked(email));
    } catch {
      fail();
    }
  }

  // Cancel a scheduled send: the draft returns to Drafts, editable again.
  async function cancelScheduledSend(emailId: string) {
    try {
      await client.cancelScheduledSend(emailId);
      afterChange(strings.sendCancelled);
      setThreadId(null);
    } catch {
      fail();
    }
  }

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

  function toggleFlag(message: Pick<EmailFull, "id" | "keywords">) {
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

  // Archive a set of messages (by id) to the Archive folder. Used by the reading
  // pane (whole open thread) and the list rows (a specific conversation).
  function archiveIds(ids: string[]) {
    const archiveBox = boxes.find((b) => b.role === "archive");
    if (archiveBox === undefined || mailboxId === null || ids.length === 0) {
      setToast(strings.archiveUnavailable);
      return;
    }
    moveIds(ids, archiveBox.id);
  }

  // Delete a set of messages: to Trash from a normal folder; permanently when
  // already in Trash (or when there is no Trash folder).
  function deleteIds(ids: string[]) {
    if (ids.length === 0 || mailboxId === null) return;
    if (ids.some((id) => currentFolderIds.includes(id))) setThreadId(null);
    const trash = boxes.find((b) => b.role === "trash");
    if (trash === undefined || mailboxId === trash.id) {
      void client.destroyMany(ids).then(() => afterChange(strings.mailDeleted)).catch(fail);
    } else {
      void client.moveMany(ids, mailboxId, trash.id).then(() => afterChange(strings.mailDeleted)).catch(fail);
    }
  }

  // Mark a set of messages seen/unseen (optimistic; the server reconciles).
  function markSeenIds(ids: string[], seen: boolean) {
    if (ids.length === 0) return;
    setReadIds((prev) => {
      const next = new Set(prev);
      ids.forEach((id) => (seen ? next.add(id) : next.delete(id)));
      return next;
    });
    void client
      .setSeenMany(ids, seen)
      .then(() => {
        emails.reload();
        mailboxes.reload();
      })
      .catch(fail);
  }

  // Snooze a set of messages until `until` (Unix seconds); a server sweeper
  // returns them to the Inbox. Closes the open thread if it's among them.
  function snoozeIds(ids: string[], until: number) {
    if (mailboxId === null || ids.length === 0) return;
    if (ids.some((id) => currentFolderIds.includes(id))) setThreadId(null);
    void client
      .snooze(ids, mailboxId, until)
      .then(() => afterChange(strings.mailSnoozed))
      .catch(fail);
  }

  function archiveThread() {
    archiveIds(currentFolderIds);
  }

  function deleteThread() {
    deleteIds(currentFolderIds);
  }

  function markThreadUnread() {
    markSeenIds(currentFolderIds, false);
  }

  // Report spam: move the conversation to Junk; when already in Junk, "Not spam"
  // moves it back to the Inbox.
  function reportSpam() {
    const current = boxes.find((b) => b.id === mailboxId);
    const junk = boxes.find((b) => b.role === "junk");
    const inbox = boxes.find((b) => b.role === "inbox");
    if (current?.role === "junk") {
      if (inbox !== undefined) moveIds(currentFolderIds, inbox.id);
    } else if (junk !== undefined) {
      moveIds(currentFolderIds, junk.id);
    } else {
      setToast(strings.junkUnavailable);
    }
  }

  // Forward the open message as an .eml attachment (a fresh "Fwd:" compose).
  function forwardAttachment() {
    if (latest === undefined) return;
    const base = (latest.subject ?? "message").replace(/[^\w.-]+/g, "_").slice(0, 60);
    setCompose({
      mode: "new",
      subject: `${strings.composeForwardPrefix}${latest.subject ?? ""}`,
      attachments: [
        { blobId: latest.blobId, type: "message/rfc822", name: `${base}.eml`, size: latest.size },
      ],
    });
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
        onSetColor={(id, color) => void setLabelColor(id, color)}
        onCreateFolder={(name, parentId) => void createFolder(name, parentId)}
        onRenameFolder={(id, name) => void renameFolder(id, name)}
        onDeleteFolder={(box) => void deleteFolder(box)}
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
        onArchive={(ts) => archiveIds(ts.flatMap((t) => t.memberIds))}
        onDelete={(ts) => deleteIds(ts.flatMap((t) => t.memberIds))}
        onMarkRead={(ts, read) => markSeenIds(ts.flatMap((t) => t.memberIds), read)}
        onSnooze={(ts, until) => snoozeIds(ts.flatMap((t) => t.memberIds), until)}
        onToggleFlag={(t) => toggleFlag(t.latest)}
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
        onReplyAll={() => latest !== undefined && setCompose({ mode: "replyAll", replyTo: latest })}
        onForward={() => latest !== undefined && setCompose({ mode: "forward", replyTo: latest })}
        onToggleFlag={() => latest !== undefined && toggleFlag(latest)}
        onArchive={archiveThread}
        onDelete={deleteThread}
        onMove={moveThread}
        onMarkUnread={markThreadUnread}
        onSnooze={(until) => snoozeIds(currentFolderIds, until)}
        onReportSpam={reportSpam}
        onForwardAttachment={forwardAttachment}
        onSmartReply={(text) =>
          latest !== undefined && setCompose({ mode: "reply", replyTo: latest, body: text })
        }
        onCancelSend={() => latest !== undefined && void cancelScheduledSend(latest.id)}
        onBlockSender={(email) => void blockSender(email)}
        isScheduled={boxes.find((b) => b.id === mailboxId)?.role === "scheduled"}
        isJunk={boxes.find((b) => b.id === mailboxId)?.role === "junk"}
      />
      {compose !== null && (
        <ComposeModal
          context={compose}
          fromEmail={identity?.email ?? ""}
          fromName={identity?.name ?? ""}
          draftsMailboxId={draftsMailboxId}
          signature={mailSettings.signature}
          orgFooter={mailSettings.orgFooter}
          onClose={() => setCompose(null)}
          onQueueSend={queueSend}
          onScheduleSend={scheduleSend}
        />
      )}
      {pendingSend !== null && (
        <div className={cx(styles.toast, styles.undoToast)} role="status">
          <span>{strings.composeUndoWindow}</span>
          <button type="button" className={styles.undoButton} onClick={undoSend}>
            {strings.composeUndoSend}
          </button>
        </div>
      )}
      {toast !== null && pendingSend === null && (
        <div className={styles.toast} role="status">
          {toast}
        </div>
      )}
    </div>
  );
}
