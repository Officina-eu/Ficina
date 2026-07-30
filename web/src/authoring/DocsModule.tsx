// Ficina Docs (ADR 0015): the technical-authoring module. Lists the caller's
// documents, opens one into the block editor, and autosaves edits through the
// tenant/owner-scoped /docs API. This is the standalone Docs surface today; it
// docks into the Collabora shell (ADR 0010) when that lands.
import { useEffect, useRef, useState } from "react";
import { ArrowLeft, Check, CloudOff, Loader2 } from "lucide-react";

import { strings } from "../i18n";
import { useJmapClient } from "../jmap";
import { cx } from "../ds";
import { type Block, type DocumentDoc, type DocumentSummary, starterBlocks } from "./document";
import { DocumentBrowser } from "./DocumentBrowser";
import { DocumentEditor } from "./DocumentEditor";
import styles from "./DocsModule.module.css";

type SaveState = "idle" | "saving" | "saved" | "error";

export function DocsModule() {
  const client = useJmapClient();
  const [documents, setDocuments] = useState<DocumentSummary[]>([]);
  const [doc, setDoc] = useState<DocumentDoc | null>(null);
  const [busy, setBusy] = useState(false);
  const [save, setSave] = useState<SaveState>("idle");
  const timer = useRef<number | undefined>(undefined);

  async function refreshList() {
    try {
      const list = await client.listDocs();
      setDocuments(list);
    } catch {
      setDocuments([]);
    }
  }

  useEffect(() => {
    void refreshList();
    return () => window.clearTimeout(timer.current);
    // client is stable for the session.
  }, [client]);

  async function persist(next: DocumentDoc) {
    setSave("saving");
    try {
      await client.saveDoc(next.id, next.title, next.blocks);
      setSave("saved");
    } catch {
      setSave("error");
    }
  }

  function schedule(next: DocumentDoc) {
    window.clearTimeout(timer.current);
    setSave("saving");
    timer.current = window.setTimeout(() => void persist(next), 700);
  }

  function mutate(next: DocumentDoc) {
    setDoc(next);
    schedule(next);
  }

  async function open(id: string) {
    setBusy(true);
    try {
      const loaded = await client.getDoc(id);
      setDoc({ ...loaded, blocks: loaded.blocks as Block[] });
      setSave("idle");
    } catch {
      // stay on the list; a transient error just leaves it unopened
    } finally {
      setBusy(false);
    }
  }

  async function create() {
    setBusy(true);
    try {
      const created = await client.createDoc("Untitled document");
      const withStarter: DocumentDoc = { ...created, blocks: starterBlocks() };
      setDoc(withStarter);
      setSave("idle");
      void persist(withStarter);
    } catch {
      // ignore; list stays
    } finally {
      setBusy(false);
    }
  }

  async function remove(id: string) {
    try {
      await client.deleteDoc(id);
    } catch {
      // ignore
    }
    if (doc?.id === id) setDoc(null);
    void refreshList();
  }

  async function back() {
    // Flush a pending save before leaving so nothing is lost.
    window.clearTimeout(timer.current);
    if (doc !== null && save !== "saved") await persist(doc);
    setDoc(null);
    void refreshList();
  }

  if (doc === null) {
    return (
      <div className={styles.app}>
        <div className={styles.canvas}>
          <DocumentBrowser
            documents={documents}
            busy={busy}
            onOpen={open}
            onCreate={create}
            onDelete={remove}
          />
        </div>
      </div>
    );
  }

  return (
    <div className={styles.app}>
      <div className={styles.bar}>
        <button type="button" className={styles.backBtn} onClick={() => void back()}>
          <ArrowLeft size={16} />
          {strings.docsAll}
        </button>
        <input
          className={styles.titleInput}
          value={doc.title}
          placeholder={strings.docsUntitled}
          onChange={(e) => mutate({ ...doc, title: e.target.value })}
          aria-label={strings.docsTitleLabel}
        />
        <span className={cx(styles.save, save === "error" && styles.saveError)}>
          {save === "saving" && <Loader2 size={14} className={styles.spin} />}
          {save === "saved" && <Check size={14} />}
          {save === "error" && <CloudOff size={14} />}
          {save === "saving"
            ? strings.docsSaving
            : save === "saved"
              ? strings.docsSaved
              : save === "error"
                ? strings.docsSaveError
                : ""}
        </span>
      </div>
      <div className={styles.canvas}>
        <article className={styles.page}>
          <DocumentEditor blocks={doc.blocks} onChange={(blocks) => mutate({ ...doc, blocks })} />
        </article>
      </div>
    </div>
  );
}
