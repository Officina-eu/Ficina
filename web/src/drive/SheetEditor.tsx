// alo Sheet — a native spreadsheet on Univer (Apache-2.0, no third-party
// branding). Like alo Doc, a sheet is a Drive node (kind "sheet") whose content
// is the editor's own JSON snapshot stored in the node's blob; opening loads it,
// edits auto-save a new version. Univer is heavy, so DriveModule code-splits this
// out — it loads only when a sheet is opened.
import { useEffect, useRef, useState } from "react";
import { X } from "lucide-react";

// Univer's own UI + engine. Framework-agnostic: it mounts into a plain DOM
// container we hand it, so we drive it from an effect rather than as JSX.
import { createUniver, LocaleType, merge, defaultTheme } from "@univerjs/presets";
import { UniverSheetsCorePreset } from "@univerjs/presets/preset-sheets-core";
import sheetsCoreEnUS from "@univerjs/presets/preset-sheets-core/locales/en-US";
import "@univerjs/presets/lib/styles/preset-sheets-core.css";

import { strings } from "../i18n";
import { useJmapClient } from "../jmap";
import { Spinner } from "../ds";
import styles from "./SheetEditor.module.css";

type SaveState = "idle" | "saving" | "saved";

/** A Univer workbook snapshot — an opaque JSON object we persist verbatim. */
type Snapshot = Record<string, unknown>;

export function SheetEditor({
  nodeId,
  name,
  onClose,
}: {
  nodeId: string;
  name: string;
  onClose: () => void;
}) {
  const client = useJmapClient();
  const containerRef = useRef<HTMLDivElement>(null);
  const apiRef = useRef<ReturnType<typeof createUniver>["univerAPI"] | null>(null);
  const disposeRef = useRef<(() => void) | null>(null);
  const lastSaved = useRef<string>("");
  const [initial, setInitial] = useState<Snapshot | null>(null);
  const [ready, setReady] = useState(false);
  const [saveState, setSaveState] = useState<SaveState>("idle");

  // Load the stored snapshot (or an empty workbook) before mounting Univer.
  useEffect(() => {
    let live = true;
    void client
      .driveSheetContent(nodeId)
      .then((data) => live && setInitial((data as Snapshot | null) ?? {}))
      .catch(() => live && setInitial({}));
    return () => {
      live = false;
    };
  }, [client, nodeId]);

  // Mount Univer once the container exists and the snapshot has loaded.
  useEffect(() => {
    if (initial === null || containerRef.current === null) return undefined;
    const { univerAPI } = createUniver({
      locale: LocaleType.EN_US,
      locales: { [LocaleType.EN_US]: merge({}, sheetsCoreEnUS) },
      theme: defaultTheme,
      presets: [UniverSheetsCorePreset({ container: containerRef.current })],
    });
    apiRef.current = univerAPI;
    // An empty object → a blank default workbook; a stored snapshot → that book.
    univerAPI.createWorkbook(Object.keys(initial).length > 0 ? initial : {});
    lastSaved.current = snapshotJson(univerAPI);
    disposeRef.current = () => univerAPI.dispose();
    setReady(true);
    return () => {
      disposeRef.current?.();
      apiRef.current = null;
      disposeRef.current = null;
    };
  }, [initial]);

  // Auto-save: poll the workbook snapshot and persist a new Drive version when it
  // changes. Polling avoids coupling to Univer's evolving event API.
  useEffect(() => {
    if (!ready) return undefined;
    const timer = window.setInterval(() => {
      const api = apiRef.current;
      if (api === null) return;
      const json = snapshotJson(api);
      if (json === "" || json === lastSaved.current) return;
      lastSaved.current = json;
      setSaveState("saving");
      void client
        .driveSaveSheet(nodeId, JSON.parse(json) as Snapshot)
        .then(() => setSaveState("saved"))
        .catch(() => setSaveState("idle"));
    }, 2500);
    return () => window.clearInterval(timer);
  }, [ready, client, nodeId]);

  function close() {
    // Flush any final edit before unmounting.
    const api = apiRef.current;
    if (api !== null) {
      const json = snapshotJson(api);
      if (json !== "" && json !== lastSaved.current) {
        void client.driveSaveSheet(nodeId, JSON.parse(json) as Snapshot);
      }
    }
    onClose();
  }

  return (
    <div className={styles.overlay}>
      <header className={styles.head}>
        <button type="button" className={styles.back} onClick={close} aria-label={strings.close}>
          <X size={18} />
        </button>
        <span className={styles.name}>{name}</span>
        <span className={styles.save}>
          {saveState === "saving" ? strings.docSaving : saveState === "saved" ? strings.docSaved : ""}
        </span>
      </header>
      <div className={styles.body}>
        {!ready && (
          <div className={styles.center}>
            <Spinner size={22} />
          </div>
        )}
        <div ref={containerRef} className={styles.univer} />
      </div>
    </div>
  );
}

/** The active workbook's snapshot as a stable JSON string, or "" if unavailable. */
function snapshotJson(api: ReturnType<typeof createUniver>["univerAPI"]): string {
  try {
    const workbook = api.getActiveWorkbook();
    if (workbook === null || workbook === undefined) return "";
    return JSON.stringify(workbook.save());
  } catch {
    return "";
  }
}
