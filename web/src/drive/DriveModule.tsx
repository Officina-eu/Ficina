// alo Drive — the file manager. Left: locations (My Files + the Spaces you
// belong to) and Trash. Right: the current folder's contents with a breadcrumb
// and per-item actions. Every file lives in one location; its access is that
// location's access (ADR 0027), so there is no per-file sharing here — sharing
// is membership of the Space it lives in, always visible via "Members".
import { Suspense, lazy, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import {
  ChevronRight,
  Copy,
  Download,
  FileText,
  FolderPlus,
  Presentation,
  Sheet,
  Table2,
  HardDrive,
  History,
  MoveRight,
  Pencil,
  Plus,
  RotateCcw,
  Trash2,
  Upload,
  Users,
} from "lucide-react";

import { strings } from "../i18n";
import { useJmapClient, type DriveNodeDto, type SpaceDto } from "../jmap";
import { Menu, Spinner, useDialogs, type MenuItem } from "../ds";
import { DestinationDialog, MembersDialog, VersionsDialog } from "./dialogs";
import { blankOfficeFile, type OfficeExt } from "./blankTemplates";
// BlockNote is heavy and only needed when a doc opens — code-split it out.
const DocEditor = lazy(() => import("./DocEditor").then((m) => ({ default: m.DocEditor })));
const BaseEditor = lazy(() => import("./BaseEditor").then((m) => ({ default: m.BaseEditor })));
const OfficeEditor = lazy(() => import("./OfficeEditor").then((m) => ({ default: m.OfficeEditor })));

/** Real Office files open in Collabora; kept here so it doesn't pull the editor
 *  into the main bundle. */
const OFFICE_EXT = /\.(docx?|xlsx?|pptx?|odt|ods|odp|rtf|csv)$/i;
import { fileSize, nodeIcon, saveBlob } from "./parts";
import styles from "./DriveModule.module.css";

type Crumb = { id: string; name: string };

export function DriveModule() {
  const client = useJmapClient();
  const { prompt, confirm } = useDialogs();

  const [spaces, setSpaces] = useState<SpaceDto[]>([]);
  const [location, setLocation] = useState<string | null>(null); // null = My Files
  const [trashView, setTrashView] = useState(false);
  const [path, setPath] = useState<Crumb[]>([]);
  const [nodes, setNodes] = useState<DriveNodeDto[] | null>(null);
  const [uploading, setUploading] = useState(false);
  const [dragOver, setDragOver] = useState(false);

  const [moveNode, setMoveNode] = useState<{ id: string; mode: "move" | "copy" } | null>(null);
  const [versionsNode, setVersionsNode] = useState<string | null>(null);
  const [openDoc, setOpenDoc] = useState<{ id: string; name: string } | null>(null);
  const [openBase, setOpenBase] = useState<{ id: string; name: string } | null>(null);
  const [openOffice, setOpenOffice] = useState<{ id: string; name: string } | null>(null);
  const [showMembers, setShowMembers] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  const parent = path.length > 0 ? (path[path.length - 1]?.id ?? null) : null;
  const currentSpace = useMemo(() => spaces.find((s) => s.id === location) ?? null, [spaces, location]);
  const canWrite = location === null || (currentSpace !== null && currentSpace.myRole !== "viewer");

  const loadSpaces = useCallback(() => {
    void client.spaces().then(setSpaces).catch(() => setSpaces([]));
  }, [client]);

  const load = useCallback(async () => {
    try {
      const list = trashView
        ? await client.driveTrash(location)
        : await client.driveList(location, parent);
      setNodes(list);
    } catch {
      setNodes([]);
    }
  }, [client, location, parent, trashView]);

  useEffect(loadSpaces, [loadSpaces]);
  useEffect(() => {
    setNodes(null);
    void load();
  }, [load]);

  // Open a node arrived at from workspace search (?open=<id>&space=<id>).
  const [searchParams, setSearchParams] = useSearchParams();
  useEffect(() => {
    const id = searchParams.get("open");
    if (id === null) return;
    const sp = searchParams.get("space");
    const next = new URLSearchParams(searchParams);
    next.delete("open");
    next.delete("space");
    setSearchParams(next, { replace: true });
    setLocation(sp);
    setTrashView(false);
    setPath([]);
    void client.driveNode(id).then((node) => {
      if (node === null) return;
      if (node.kind === "folder") setPath([{ id: node.id, name: node.name }]);
      else if (node.kind === "doc") setOpenDoc({ id, name: node.name });
      else if (node.kind === "base") setOpenBase({ id, name: node.name });
      else if (node.kind === "file" && OFFICE_EXT.test(node.name)) setOpenOffice({ id, name: node.name });
    });
  }, [searchParams, setSearchParams, client]);

  function selectLocation(space: string | null) {
    setLocation(space);
    setTrashView(false);
    setPath([]);
  }

  function openNode(n: DriveNodeDto) {
    if (n.kind === "folder") setPath((p) => [...p, { id: n.id, name: n.name }]);
    else if (n.kind === "doc") setOpenDoc({ id: n.id, name: n.name });
    else if (n.kind === "base") setOpenBase({ id: n.id, name: n.name });
    else if (n.kind === "file" && OFFICE_EXT.test(n.name)) setOpenOffice({ id: n.id, name: n.name });
    else void download(n);
  }

  async function newDoc() {
    const name = (await prompt({ message: strings.driveNewDocPrompt }))?.trim();
    if (!name) return;
    try {
      const id = await client.driveCreateDoc(location, parent, name);
      await load();
      setOpenDoc({ id, name });
    } catch {
      /* ignore */
    }
  }

  async function newBase() {
    const name = (await prompt({ message: strings.driveNewBasePrompt }))?.trim();
    if (!name) return;
    try {
      const id = await client.createBase(location, parent, name);
      await load();
      setOpenBase({ id, name });
    } catch {
      /* ignore */
    }
  }

  /** Create a blank Office document (Word/Excel/PowerPoint) from a template and
   *  open it in the Collabora editor — the two-file-types rule (ADR 0030). */
  async function newOffice(ext: OfficeExt) {
    const kind = ext === "xlsx" ? strings.driveKindExcel : strings.driveKindSlides;
    const name = (await prompt({ message: strings.driveNameNew(kind) }))?.trim();
    if (!name) return;
    try {
      const file = blankOfficeFile(ext, name);
      const id = await client.driveUpload(location, parent, file);
      await load();
      setOpenOffice({ id, name: file.name });
    } catch {
      /* ignore */
    }
  }

  async function download(n: DriveNodeDto) {
    if (n.blobId === null) return;
    try {
      saveBlob(await client.driveDownload(n.id), n.name);
    } catch {
      /* ignore */
    }
  }

  async function uploadFiles(files: FileList | File[]) {
    setUploading(true);
    try {
      for (const f of Array.from(files)) {
        await client.driveUpload(location, parent, f);
      }
      await load();
    } catch {
      /* leave as-is */
    } finally {
      setUploading(false);
    }
  }

  async function newFolder() {
    const name = (await prompt({ message: strings.driveNewFolderPrompt }))?.trim();
    if (!name) return;
    try {
      await client.driveCreateFolder(location, parent, name);
      await load();
    } catch {
      /* ignore */
    }
  }

  async function newSpace() {
    const name = (await prompt({ message: strings.driveNewSpacePrompt }))?.trim();
    if (!name) return;
    try {
      const id = await client.createSpace(name);
      loadSpaces();
      selectLocation(id);
    } catch {
      /* ignore */
    }
  }

  async function rename(n: DriveNodeDto) {
    const name = (await prompt({ message: strings.driveRenamePrompt, defaultValue: n.name }))?.trim();
    if (!name || name === n.name) return;
    try {
      await client.driveRename(n.id, name);
      await load();
    } catch {
      /* ignore */
    }
  }

  async function trash(n: DriveNodeDto) {
    if (!(await confirm({ message: strings.driveTrashConfirm(n.name) }))) return;
    try {
      await client.driveTrashNode(n.id);
      await load();
    } catch {
      /* ignore */
    }
  }

  async function restore(n: DriveNodeDto) {
    try {
      await client.driveRestoreNode(n.id);
      await load();
    } catch {
      /* ignore */
    }
  }

  async function purge(n: DriveNodeDto) {
    if (!(await confirm({ message: strings.drivePurgeConfirm(n.name), danger: true }))) return;
    try {
      await client.drivePurge(n.id);
      await load();
    } catch {
      /* ignore */
    }
  }

  async function pickedDestination(space: string | null) {
    const target = moveNode;
    setMoveNode(null);
    if (target === null) return;
    try {
      if (target.mode === "move") await client.driveMove(target.id, space, null);
      else await client.driveCopy(target.id, space, null);
      await load();
    } catch {
      /* ignore */
    }
  }

  function rowMenu(n: DriveNodeDto): MenuItem[] {
    if (trashView) {
      return [
        { key: "restore", label: strings.driveRestore, icon: <RotateCcw size={15} />, onClick: () => void restore(n) },
        { key: "purge", label: strings.driveDeleteForever, icon: <Trash2 size={15} />, danger: true, onClick: () => void purge(n) },
      ];
    }
    const items: MenuItem[] = [];
    if (n.kind !== "folder") {
      items.push({ key: "download", label: strings.driveDownload, icon: <Download size={15} />, onClick: () => void download(n) });
      items.push({ key: "versions", label: strings.driveVersionHistory, icon: <History size={15} />, onClick: () => setVersionsNode(n.id) });
    }
    if (canWrite) {
      items.push({ key: "rename", label: strings.driveRename, icon: <Pencil size={15} />, onClick: () => void rename(n) });
      items.push({ key: "move", label: strings.driveMove, icon: <MoveRight size={15} />, onClick: () => setMoveNode({ id: n.id, mode: "move" }) });
      items.push({ key: "copy", label: strings.driveCopy, icon: <Copy size={15} />, onClick: () => setMoveNode({ id: n.id, mode: "copy" }) });
      items.push({ key: "trash", label: strings.driveTrashAction, icon: <Trash2 size={15} />, danger: true, onClick: () => void trash(n) });
    }
    return items;
  }

  return (
    <div className={styles.drive}>
      <aside className={styles.sidebar}>
        <div className={styles.sideGroup}>
          <button
            type="button"
            className={location === null && !trashView ? `${styles.sideItem} ${styles.sideActive}` : styles.sideItem}
            onClick={() => selectLocation(null)}
          >
            <HardDrive size={18} />
            <span>{strings.driveMyFiles}</span>
          </button>
        </div>

        <div className={styles.sideGroup}>
          <div className={styles.sideLabel}>
            {strings.driveSpaces}
            <button type="button" className={styles.sideAdd} onClick={() => void newSpace()} aria-label={strings.driveNewSpace}>
              <Plus size={14} />
            </button>
          </div>
          {spaces.filter((s) => !s.archived).map((s) => (
            <button
              key={s.id}
              type="button"
              className={location === s.id && !trashView ? `${styles.sideItem} ${styles.sideActive}` : styles.sideItem}
              onClick={() => selectLocation(s.id)}
            >
              <Users size={18} />
              <span className={styles.sideName}>{s.name}</span>
            </button>
          ))}
        </div>

        <div className={`${styles.sideGroup} ${styles.sideBottom}`}>
          <button
            type="button"
            className={trashView ? `${styles.sideItem} ${styles.sideActive}` : styles.sideItem}
            onClick={() => {
              setTrashView(true);
              setPath([]);
            }}
          >
            <Trash2 size={18} />
            <span>{strings.driveTrash}</span>
          </button>
        </div>
      </aside>

      <section
        className={dragOver ? `${styles.main} ${styles.mainDrag}` : styles.main}
        onDragOver={(e) => {
          if (canWrite && !trashView) {
            e.preventDefault();
            setDragOver(true);
          }
        }}
        onDragLeave={() => setDragOver(false)}
        onDrop={(e) => {
          setDragOver(false);
          if (canWrite && !trashView && e.dataTransfer.files.length > 0) {
            e.preventDefault();
            void uploadFiles(e.dataTransfer.files);
          }
        }}
      >
        <header className={styles.head}>
          <nav className={styles.crumbs}>
            <button type="button" className={styles.crumb} onClick={() => setPath([])}>
              {trashView ? strings.driveTrash : currentSpace?.name ?? strings.driveMyFiles}
            </button>
            {path.map((c, i) => (
              <span key={c.id} className={styles.crumbSep}>
                <ChevronRight size={14} />
                <button type="button" className={styles.crumb} onClick={() => setPath(path.slice(0, i + 1))}>
                  {c.name}
                </button>
              </span>
            ))}
          </nav>
          <div className={styles.actions}>
            {currentSpace !== null && !trashView && (
              <button type="button" className={styles.ghostBtn} onClick={() => setShowMembers(true)}>
                <Users size={15} /> {strings.driveMembers}
              </button>
            )}
            {canWrite && !trashView && (
              <>
                <Menu
                  triggerLabel={strings.driveNew}
                  label={strings.driveNew}
                  icon={<Plus size={15} />}
                  align="end"
                  items={[
                    { key: "doc", label: strings.driveKindDoc, icon: <FileText size={15} />, onClick: () => void newDoc() },
                    { key: "base", label: strings.driveKindSheet, icon: <Table2 size={15} />, onClick: () => void newBase() },
                    { key: "excel", label: strings.driveKindExcel, icon: <Sheet size={15} />, onClick: () => void newOffice("xlsx"), divider: true },
                    { key: "slides", label: strings.driveKindSlides, icon: <Presentation size={15} />, onClick: () => void newOffice("pptx") },
                    { key: "folder", label: strings.driveKindFolder, icon: <FolderPlus size={15} />, onClick: () => void newFolder(), divider: true },
                  ]}
                />
                <button type="button" className={styles.primaryBtn} onClick={() => fileRef.current?.click()} disabled={uploading}>
                  <Upload size={15} /> {uploading ? strings.driveUploading : strings.driveUpload}
                </button>
                <input
                  ref={fileRef}
                  type="file"
                  multiple
                  style={{ display: "none" }}
                  onChange={(e) => {
                    if (e.target.files && e.target.files.length > 0) void uploadFiles(e.target.files);
                    e.target.value = "";
                  }}
                />
              </>
            )}
          </div>
        </header>

        {nodes === null ? (
          <div className={styles.center}>
            <Spinner size={22} />
          </div>
        ) : nodes.length === 0 ? (
          <div className={styles.empty}>
            {trashView ? strings.driveEmptyTrash : strings.driveEmpty}
          </div>
        ) : (
          <ul className={styles.list}>
            <li className={styles.listHead}>
              <span className={styles.colName}>{strings.driveColName}</span>
              <span className={styles.colSize}>{strings.driveColSize}</span>
              <span className={styles.colDate}>{strings.driveColModified}</span>
              <span className={styles.colMenu} />
            </li>
            {nodes.map((n) => {
              const Icon = nodeIcon(n);
              return (
                <li key={n.id} className={styles.row}>
                  <button
                    type="button"
                    className={styles.rowMain}
                    onClick={() => openNode(n)}
                    onDoubleClick={() => openNode(n)}
                  >
                    <Icon size={18} className={n.kind === "folder" ? styles.folderIcon : styles.fileIcon} />
                    <span className={styles.rowName}>{n.name}</span>
                  </button>
                  <span className={styles.colSize}>{n.kind === "folder" ? "—" : fileSize(n.size)}</span>
                  <span className={styles.colDate}>{new Date(n.updatedAt).toLocaleDateString()}</span>
                  <span className={styles.colMenu}>
                    <Menu label={strings.driveActions} icon={<span aria-hidden>⋯</span>} items={rowMenu(n)} />
                  </span>
                </li>
              );
            })}
          </ul>
        )}
      </section>

      {moveNode !== null && (
        <DestinationDialog
          spaces={spaces}
          mode={moveNode.mode}
          onPick={(s) => void pickedDestination(s)}
          onClose={() => setMoveNode(null)}
        />
      )}
      {versionsNode !== null && (
        <VersionsDialog nodeId={versionsNode} onChanged={() => void load()} onClose={() => setVersionsNode(null)} />
      )}
      {showMembers && currentSpace !== null && (
        <MembersDialog space={currentSpace} onClose={() => setShowMembers(false)} />
      )}
      {openDoc !== null && (
        <Suspense fallback={null}>
          <DocEditor
            nodeId={openDoc.id}
            name={openDoc.name}
            onClose={() => {
              setOpenDoc(null);
              void load();
            }}
          />
        </Suspense>
      )}
      {openBase !== null && (
        <Suspense fallback={null}>
          <BaseEditor
            nodeId={openBase.id}
            name={openBase.name}
            onClose={() => {
              setOpenBase(null);
              void load();
            }}
          />
        </Suspense>
      )}
      {openOffice !== null && (
        <Suspense fallback={null}>
          <OfficeEditor
            nodeId={openOffice.id}
            name={openOffice.name}
            onClose={() => {
              setOpenOffice(null);
              void load();
            }}
          />
        </Suspense>
      )}
    </div>
  );
}
