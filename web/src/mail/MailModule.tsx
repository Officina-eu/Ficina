// The Mail module: the four-column surface (rail is the shell's; here folders ·
// messages · reading pane) and the state tying them together — selection,
// optimistic read/flag state, real flag + archive actions, and honest "coming
// soon" toasts for compose/reply/AI (which have no backend yet). It renders
// inside <AppShell>'s main area, nothing more.
import { useEffect, useState } from "react";
import type { CSSProperties } from "react";

import { strings } from "../i18n";
import { ResizeHandle, usePanelWidth } from "../ds";
import { KEYWORD_FLAGGED, useJmapClient } from "../jmap";
import type { EmailFull, EmailHeaders } from "../jmap";
import { useAuth } from "../auth";
import { useEmailBody, useEmailHeaders, useMailboxes } from "./state/useMail";
import { isUnread } from "./format";
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
  const [emailId, setEmailId] = useState<string | null>(null);
  const [readIds, setReadIds] = useState<ReadonlySet<string>>(new Set());
  const [flags, setFlags] = useState<ReadonlyMap<string, boolean>>(new Map());
  const [toast, setToast] = useState<string | null>(null);
  const [compose, setCompose] = useState<ComposeContext | null>(null);

  const emails = useEmailHeaders(mailboxId);
  const email = useEmailBody(emailId);

  const boxes = mailboxes.status === "ready" ? (mailboxes.data ?? []) : [];
  const folderName = boxes.find((b) => b.id === mailboxId)?.name ?? strings.moduleMail;
  const draftsMailboxId =
    boxes.find((b) => b.role === "drafts")?.id ?? mailboxId ?? boxes[0]?.id ?? null;

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

  function openMailbox(id: string) {
    setMailboxId(id);
    setEmailId(null);
  }

  function openEmail(header: EmailHeaders) {
    setEmailId(header.id);
    if (isUnread(header) && !readIds.has(header.id)) {
      setReadIds((prev) => new Set(prev).add(header.id));
      void client.setSeen(header.id, true).catch(() => {
        // Non-fatal; the server stays the source of truth and corrects on
        // the next folder load.
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

  function archive(message: EmailFull) {
    const archiveBox = boxes.find((b) => b.role === "archive");
    if (archiveBox === undefined || mailboxId === null) {
      setToast(strings.archiveUnavailable);
      return;
    }
    void client
      .move(message.id, mailboxId, archiveBox.id)
      .then(() => {
        setEmailId(null);
        emails.reload();
        mailboxes.reload();
      })
      .catch(() => setToast(strings.archiveUnavailable));
  }

  function openReply() {
    if (email.data !== null) setCompose({ mode: "reply", replyTo: email.data });
  }

  function onSent() {
    setCompose(null);
    setToast(strings.composeSent);
    emails.reload();
    mailboxes.reload();
  }

  const widthVars = {
    "--sidebar-width": `${folders.width}px`,
    "--list-width": `${list.width}px`,
  } as CSSProperties;

  return (
    <div className={styles.mail} style={widthVars}>
      <FolderSidebar
        mailboxes={mailboxes}
        selectedId={mailboxId}
        onSelect={openMailbox}
        onCompose={() => setCompose({ mode: "new" })}
      />
      <ResizeHandle
        ariaLabel={strings.resizeFolders}
        onResize={folders.applyDelta}
        onCommit={folders.commit}
        onReset={folders.reset}
      />
      <MessageList
        folderName={folderName}
        emails={emails}
        selectedId={emailId}
        readIds={readIds}
        flagOverrides={flags}
        onSelect={openEmail}
      />
      <ResizeHandle
        ariaLabel={strings.resizeMessages}
        onResize={list.applyDelta}
        onCommit={list.commit}
        onReset={list.reset}
      />
      <ReadingPane
        email={email}
        mailboxes={boxes}
        flagOverrides={flags}
        onReply={openReply}
        onToggleFlag={toggleFlag}
        onArchive={archive}
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
