// The Mail module: the three-pane surface (folders · messages · reading pane)
// and the selection state tying them together. It is the first real tenant of
// the shell frame — it renders inside <AppShell>'s main area, nothing more.
import { useEffect, useState } from "react";

import { useJmapClient } from "../jmap";
import type { EmailHeaders } from "../jmap";
import { useEmailBody, useEmailHeaders, useMailboxes } from "./state/useMail";
import { isUnread } from "./format";
import { MailboxList } from "./components/MailboxList";
import { MessageList } from "./components/MessageList";
import { ReadingPane } from "./components/ReadingPane";
import styles from "./MailModule.module.css";

export function MailModule() {
  const client = useJmapClient();
  const mailboxes = useMailboxes();

  const [mailboxId, setMailboxId] = useState<string | null>(null);
  const [emailId, setEmailId] = useState<string | null>(null);
  const [readIds, setReadIds] = useState<ReadonlySet<string>>(new Set());

  const emails = useEmailHeaders(mailboxId);
  const email = useEmailBody(emailId);

  // Default to the Inbox (or the first mailbox) once folders load.
  useEffect(() => {
    if (mailboxId !== null || mailboxes.status !== "ready") return;
    const list = mailboxes.data ?? [];
    const inbox = list.find((m) => m.role === "inbox") ?? list[0];
    if (inbox !== undefined) setMailboxId(inbox.id);
  }, [mailboxId, mailboxes.status, mailboxes.data]);

  function openMailbox(id: string) {
    setMailboxId(id);
    setEmailId(null);
  }

  function openEmail(header: EmailHeaders) {
    setEmailId(header.id);
    if (isUnread(header) && !readIds.has(header.id)) {
      setReadIds((prev) => new Set(prev).add(header.id));
      void client.setSeen(header.id, true).catch(() => {
        // A failed read-marking is non-fatal; the server stays the source of
        // truth and the state corrects on the next folder load.
      });
    }
  }

  return (
    <div className={styles.mail}>
      <MailboxList mailboxes={mailboxes} selectedId={mailboxId} onSelect={openMailbox} />
      <MessageList emails={emails} selectedId={emailId} readIds={readIds} onSelect={openEmail} />
      <ReadingPane email={email} />
    </div>
  );
}
