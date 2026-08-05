// alo Doc — the block editor (ADR 0031), built on BlockNote. A doc's content is
// a BlockNote block tree stored as the node's blob in Drive; opening loads it,
// editing auto-saves a new version (debounced). This is the v1 document: a real
// Notion-style block editor over Drive storage. Technical/interactive/linked
// blocks and propose-then-approve AI are later slices.
import { useCallback, useEffect, useRef, useState, type ComponentProps } from "react";
import { X } from "lucide-react";
import { useCreateBlockNote } from "@blocknote/react";
import { BlockNoteView } from "@blocknote/mantine";
import "@blocknote/core/fonts/inter.css";
import "@blocknote/mantine/style.css";

import { strings } from "../i18n";
import { useJmapClient } from "../jmap";
import { Spinner } from "../ds";
import styles from "./DocEditor.module.css";

type SaveState = "idle" | "saving" | "saved";

/** The editor proper — mounted only once content is loaded. The doc's blocks
 *  are loaded into a default-schema editor via `replaceBlocks` (casting at the
 *  BlockNote boundary, whose generics fight `exactOptionalPropertyTypes`). */
function Editor({
  initial,
  onChange,
}: {
  initial: unknown[];
  onChange: (blocks: unknown[]) => void;
}) {
  const editor = useCreateBlockNote();
  const loaded = useRef(false);
  useEffect(() => {
    if (!loaded.current && initial.length > 0) {
      loaded.current = true;
      editor.replaceBlocks(
        editor.document,
        initial as Parameters<typeof editor.replaceBlocks>[1],
      );
    }
  }, [editor, initial]);
  // BlockNote's own editor type does not satisfy `exactOptionalPropertyTypes`;
  // cast to the component's expected prop type (same library, same version).
  const editorProp = editor as unknown as ComponentProps<typeof BlockNoteView>["editor"];
  return <BlockNoteView editor={editorProp} onChange={() => onChange(editor.document)} />;
}

export function DocEditor({
  nodeId,
  name,
  onClose,
}: {
  nodeId: string;
  name: string;
  onClose: () => void;
}) {
  const client = useJmapClient();
  const [initial, setInitial] = useState<unknown[] | null>(null);
  const [saveState, setSaveState] = useState<SaveState>("idle");
  const pending = useRef<unknown[] | null>(null);
  const timer = useRef<number | null>(null);

  useEffect(() => {
    let live = true;
    void client
      .driveDocContent(nodeId)
      .then((c) => {
        if (live) setInitial(c as unknown[]);
      })
      .catch(() => {
        if (live) setInitial([]);
      });
    return () => {
      live = false;
    };
  }, [client, nodeId]);

  const save = useCallback(
    async (blocks: unknown[]) => {
      setSaveState("saving");
      try {
        await client.driveSaveDoc(nodeId, blocks);
        pending.current = null;
        setSaveState("saved");
      } catch {
        setSaveState("idle");
      }
    },
    [client, nodeId],
  );

  const onChange = useCallback(
    (blocks: unknown[]) => {
      pending.current = blocks;
      setSaveState("saving");
      if (timer.current !== null) window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => {
        if (pending.current) void save(pending.current);
      }, 1200);
    },
    [save],
  );

  async function close() {
    if (timer.current !== null) window.clearTimeout(timer.current);
    if (pending.current) await save(pending.current);
    onClose();
  }

  return (
    <div className={styles.overlay}>
      <header className={styles.head}>
        <button
          type="button"
          className={styles.back}
          onClick={() => void close()}
          aria-label={strings.close}
        >
          <X size={18} />
        </button>
        <span className={styles.name}>{name}</span>
        <span className={styles.save}>
          {saveState === "saving"
            ? strings.docSaving
            : saveState === "saved"
              ? strings.docSaved
              : ""}
        </span>
      </header>
      <div className={styles.body}>
        {initial === null ? (
          <div className={styles.center}>
            <Spinner size={22} />
          </div>
        ) : (
          <Editor initial={initial} onChange={onChange} />
        )}
      </div>
    </div>
  );
}
