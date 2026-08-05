// Workspace search (ADR 0029) — a global command-palette-style overlay that
// searches across the caller's files and tasks by name/title. Opened from the
// rail or Ctrl/Cmd-K; a result navigates to its module and opens it. Content
// search and more modules (mail, contacts) come in later slices.
import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  FileText,
  Folder,
  File as FileIcon,
  ListChecks,
  Search,
  Table2,
  X,
  type LucideIcon,
} from "lucide-react";

import { strings } from "../i18n";
import { useJmapClient, type SearchHitDto } from "../jmap";
import { Spinner } from "../ds";
import styles from "./SearchOverlay.module.css";

function hitIcon(kind: string): LucideIcon {
  switch (kind) {
    case "folder":
      return Folder;
    case "doc":
      return FileText;
    case "base":
      return Table2;
    case "task":
      return ListChecks;
    default:
      return FileIcon;
  }
}

export function SearchOverlay({ onClose }: { onClose: () => void }) {
  const client = useJmapClient();
  const navigate = useNavigate();
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<SearchHitDto[] | null>(null);
  const [loading, setLoading] = useState(false);
  const timer = useRef<number | null>(null);

  useEffect(() => {
    if (q.trim() === "") {
      setHits(null);
      setLoading(false);
      return undefined;
    }
    setLoading(true);
    if (timer.current !== null) window.clearTimeout(timer.current);
    let live = true;
    timer.current = window.setTimeout(() => {
      void client
        .search(q)
        .then((h) => live && setHits(h))
        .catch(() => live && setHits([]))
        .finally(() => live && setLoading(false));
    }, 200);
    return () => {
      live = false;
      if (timer.current !== null) window.clearTimeout(timer.current);
    };
  }, [q, client]);

  useEffect(() => {
    function key(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("keydown", key);
    return () => document.removeEventListener("keydown", key);
  }, [onClose]);

  function open(hit: SearchHitDto) {
    if (hit.kind === "task") {
      navigate(`/tasks?open=${encodeURIComponent(hit.id)}`);
    } else {
      const space = hit.space ? `&space=${encodeURIComponent(hit.space)}` : "";
      navigate(`/drive?open=${encodeURIComponent(hit.id)}${space}`);
    }
    onClose();
  }

  return (
    <div className={styles.scrim} onMouseDown={onClose}>
      <div className={styles.panel} onMouseDown={(e) => e.stopPropagation()}>
        <div className={styles.searchRow}>
          <Search size={18} className={styles.searchIcon} />
          <input
            className={styles.input}
            autoFocus
            value={q}
            placeholder={strings.searchPlaceholder}
            onChange={(e) => setQ(e.target.value)}
          />
          <button type="button" className={styles.close} onClick={onClose} aria-label={strings.close}>
            <X size={18} />
          </button>
        </div>
        <div className={styles.results}>
          {loading && hits === null ? (
            <div className={styles.state}>
              <Spinner size={18} />
            </div>
          ) : hits === null ? (
            <div className={styles.state}>{strings.searchHint}</div>
          ) : hits.length === 0 ? (
            <div className={styles.state}>{strings.searchNoResults}</div>
          ) : (
            <ul className={styles.list}>
              {hits.map((h) => {
                const Icon = hitIcon(h.kind);
                return (
                  <li key={`${h.kind}:${h.id}`}>
                    <button type="button" className={styles.hit} onClick={() => open(h)}>
                      <Icon size={16} className={styles.hitIcon} />
                      <span className={styles.hitTitle}>{h.title}</span>
                      <span className={styles.hitKind}>{strings.searchKind(h.kind)}</span>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
}
