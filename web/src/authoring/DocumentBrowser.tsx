// The document browser (ADR 0015): the caller's technical documents — create,
// open, delete. Metadata only; opening loads the blocks.
import { FileText, Plus, Trash2 } from "lucide-react";

import { strings } from "../i18n";
import type { DocumentSummary } from "./document";
import styles from "./DocumentBrowser.module.css";

interface DocumentBrowserProps {
  documents: DocumentSummary[];
  busy: boolean;
  onOpen: (id: string) => void;
  onCreate: () => void;
  onDelete: (id: string) => void;
}

/** A friendlier timestamp than the raw Postgres text (drops the seconds/zone). */
function shortDate(iso: string): string {
  // "2026-07-30 15:30:00+00" → "2026-07-30 15:30"
  return iso.replace(/:\d\d[+-]\d\d$/, "").replace("T", " ");
}

export function DocumentBrowser({
  documents,
  busy,
  onOpen,
  onCreate,
  onDelete,
}: DocumentBrowserProps) {
  return (
    <div className={styles.browser}>
      <div className={styles.head}>
        <h1 className={styles.title}>{strings.docsTitle}</h1>
        <button type="button" className={styles.newBtn} onClick={onCreate} disabled={busy}>
          <Plus size={16} />
          {strings.docsNew}
        </button>
      </div>

      {documents.length === 0 ? (
        <div className={styles.empty}>
          <FileText size={28} className={styles.emptyIcon} />
          <p className={styles.emptyText}>{strings.docsEmpty}</p>
          <button type="button" className={styles.newBtn} onClick={onCreate} disabled={busy}>
            <Plus size={16} />
            {strings.docsNew}
          </button>
        </div>
      ) : (
        <ul className={styles.list}>
          {documents.map((d) => (
            <li key={d.id} className={styles.item}>
              <button type="button" className={styles.open} onClick={() => onOpen(d.id)}>
                <FileText size={18} className={styles.itemIcon} />
                <span className={styles.itemTitle}>{d.title}</span>
                <span className={styles.itemDate}>{shortDate(d.updatedAt)}</span>
              </button>
              <button
                type="button"
                className={styles.delete}
                onClick={() => onDelete(d.id)}
                aria-label={strings.docsDelete(d.title)}
                title={strings.docsDelete(d.title)}
              >
                <Trash2 size={16} />
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
