// alo Sheet — a native spreadsheet on Univer (Apache-2.0, no third-party
// branding). Like alo Doc, a sheet is a Drive node (kind "sheet") whose content
// is the editor's own JSON snapshot stored in the node's blob; opening loads it,
// edits auto-save a new version. Univer is heavy, so DriveModule code-splits this
// out — it loads only when a sheet is opened.
import { useEffect, useRef, useState } from "react";
import { Check, ChevronLeft, Download, MoreHorizontal, Pencil, X } from "lucide-react";

// Univer's own UI + engine. Framework-agnostic: it mounts into a plain DOM
// container we hand it, so we drive it from an effect rather than as JSX.
import { createUniver, LocaleType, merge, defaultTheme } from "@univerjs/presets";
import { UniverSheetsCorePreset } from "@univerjs/presets/preset-sheets-core";
import sheetsCoreEnUS from "@univerjs/presets/preset-sheets-core/locales/en-US";
import "@univerjs/presets/lib/styles/preset-sheets-core.css";

import { strings } from "../i18n";
import { useJmapClient } from "../jmap";
import { Menu, Spinner } from "../ds";
import { saveBlob } from "./parts";
import { univerSnapshotToXlsx } from "./exportOffice";
import styles from "./SheetEditor.module.css";

const XLSX_MIME = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";

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
  // Editable sheet name (persisted via driveRename); the grid filename tracks it.
  const [sheetName, setSheetName] = useState(name);
  const nameRef = useRef<HTMLInputElement>(null);

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

  /** Persist a renamed sheet (Drive rename); revert on empty or failure. */
  function commitName() {
    const trimmed = sheetName.trim();
    if (trimmed === "" ) {
      setSheetName(name);
      return;
    }
    if (trimmed !== name) {
      void client.driveRename(nodeId, trimmed).catch(() => setSheetName(name));
    }
  }

  /** Export the live workbook as a real `.xlsx` and download it (ADR 0033) — the
   *  round-trip that lets an alo Sheet leave as a genuine Excel file. */
  function downloadXlsx() {
    const api = apiRef.current;
    if (api === null) return;
    const json = snapshotJson(api);
    if (json === "") return;
    const bytes = univerSnapshotToXlsx(
      JSON.parse(json) as Parameters<typeof univerSnapshotToXlsx>[0],
    );
    saveBlob(new Blob([bytes as BlobPart], { type: XLSX_MIME }), `${sheetName.trim() || name}.xlsx`);
  }

  return (
    <div className={styles.overlay}>
      <header className={styles.head}>
        <button type="button" className={styles.back} onClick={close} aria-label={strings.close} title={strings.close}>
          <ChevronLeft size={18} />
        </button>
        <input
          ref={nameRef}
          className={styles.nameInput}
          value={sheetName}
          aria-label={strings.sheetName}
          onChange={(e) => setSheetName(e.target.value)}
          onBlur={commitName}
          onKeyDown={(e) => {
            if (e.key === "Enter") e.currentTarget.blur();
            else if (e.key === "Escape") {
              setSheetName(name);
              e.currentTarget.blur();
            }
          }}
        />
        <span className={styles.saved} aria-live="polite">
          {saveState === "saving" ? (
            <>
              <Spinner size={12} /> {strings.docSaving}
            </>
          ) : (
            <>
              <Check size={14} className={styles.savedIcon} /> {strings.sheetSaved}
            </>
          )}
        </span>
        <div className={styles.grow} />
        <button
          type="button"
          className={styles.export}
          onClick={downloadXlsx}
          disabled={!ready}
          title={strings.sheetDownloadXlsx}
        >
          <Download size={16} />
          <span>{strings.sheetExport}</span>
        </button>
        <Menu
          label={strings.sheetMore}
          icon={<MoreHorizontal size={18} />}
          align="end"
          items={[
            {
              key: "rename",
              label: strings.driveRename,
              icon: <Pencil size={15} />,
              onClick: () => {
                nameRef.current?.focus();
                nameRef.current?.select();
              },
            },
            {
              key: "export",
              label: strings.sheetDownloadXlsx,
              icon: <Download size={15} />,
              onClick: downloadXlsx,
            },
            {
              key: "close",
              label: strings.close,
              icon: <X size={15} />,
              onClick: close,
              divider: true,
            },
          ]}
        />
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
