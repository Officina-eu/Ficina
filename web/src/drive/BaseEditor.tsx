// alo Base — the grid editor (ADR 0032). A relational table with a spreadsheet
// face: columns are typed fields, rows are records, and edits save per cell.
// This is the grid view; board/calendar/gallery over the same records are later
// slices (a custom lightweight grid for now — AG Grid/Univer become worth it at
// scale, per the ADR). Every write is gated server-side by the Base's Drive
// access, so this UI just reflects what the caller may do.
import { useCallback, useEffect, useRef, useState } from "react";
import { Plus, Table2, X } from "lucide-react";

import { strings } from "../i18n";
import {
  useJmapClient,
  type BaseDto,
  type BaseFieldDto,
  type BaseFieldType,
  type BaseRecordDto,
  type BaseTableDto,
} from "../jmap";
import { Spinner } from "../ds";
import styles from "./BaseEditor.module.css";

/** The field types slice-1 renders as fully-editable cells. */
const EDITABLE_TYPES: { type: BaseFieldType; label: () => string }[] = [
  { type: "text", label: () => strings.baseTypeText },
  { type: "number", label: () => strings.baseTypeNumber },
  { type: "date", label: () => strings.baseTypeDate },
  { type: "checkbox", label: () => strings.baseTypeCheckbox },
];

export function BaseEditor({
  nodeId,
  name,
  onClose,
}: {
  nodeId: string;
  name: string;
  onClose: () => void;
}) {
  const client = useJmapClient();
  const [base, setBase] = useState<BaseDto | null>(null);
  const [activeTable, setActiveTable] = useState(0);
  const [saveState, setSaveState] = useState<"idle" | "saving" | "saved">("idle");
  const [addingField, setAddingField] = useState(false);
  const [newFieldName, setNewFieldName] = useState("");
  const [newFieldType, setNewFieldType] = useState<BaseFieldType>("text");
  const addWrapRef = useRef<HTMLDivElement>(null);

  const reload = useCallback(async () => {
    try {
      setBase(await client.base(nodeId));
    } catch {
      setBase({ nodeId, tables: [] });
    }
  }, [client, nodeId]);

  useEffect(() => {
    void reload();
  }, [reload]);

  useEffect(() => {
    if (!addingField) return undefined;
    function down(e: PointerEvent) {
      if (addWrapRef.current && !addWrapRef.current.contains(e.target as Node)) {
        setAddingField(false);
      }
    }
    document.addEventListener("pointerdown", down);
    return () => document.removeEventListener("pointerdown", down);
  }, [addingField]);

  const table: BaseTableDto | undefined = base?.tables[activeTable];

  /** Optimistically write a cell and persist the whole record. */
  async function setCell(record: BaseRecordDto, fieldId: string, value: unknown) {
    const cells = { ...record.cells, [fieldId]: value };
    setBase((b) => {
      if (b === null) return b;
      const tables = b.tables.map((t, i) =>
        i !== activeTable
          ? t
          : { ...t, records: t.records.map((r) => (r.id === record.id ? { ...r, cells } : r)) },
      );
      return { ...b, tables };
    });
    setSaveState("saving");
    try {
      await client.baseUpdateRecord(record.id, cells);
      setSaveState("saved");
    } catch {
      setSaveState("idle");
      void reload();
    }
  }

  async function addRow() {
    if (!table) return;
    try {
      await client.baseAddRecord(table.id);
      await reload();
    } catch {
      /* ignore */
    }
  }

  async function addField() {
    const nm = newFieldName.trim();
    if (nm === "" || !table) return;
    try {
      await client.baseAddField(table.id, nm, newFieldType);
      setNewFieldName("");
      setNewFieldType("text");
      setAddingField(false);
      await reload();
    } catch {
      /* ignore */
    }
  }

  async function addTable() {
    if (!base) return;
    const newIndex = base.tables.length; // the new table lands at the end
    try {
      await client.baseAddTable(nodeId, `Table ${base.tables.length + 1}`);
      await reload();
      setActiveTable(newIndex);
    } catch {
      /* ignore */
    }
  }

  return (
    <div className={styles.overlay}>
      <header className={styles.head}>
        <button type="button" className={styles.back} onClick={onClose} aria-label={strings.close}>
          <X size={18} />
        </button>
        <span className={styles.name}>{name}</span>
        <span className={styles.save}>
          {saveState === "saving" ? strings.docSaving : saveState === "saved" ? strings.docSaved : ""}
        </span>
      </header>

      {base === null ? (
        <div className={styles.center}>
          <Spinner size={22} />
        </div>
      ) : (
        <>
          <div className={styles.tabs}>
            {base.tables.map((t, i) => (
              <button
                key={t.id}
                type="button"
                className={i === activeTable ? `${styles.tab} ${styles.tabOn}` : styles.tab}
                onClick={() => setActiveTable(i)}
              >
                <Table2 size={14} /> {t.name}
              </button>
            ))}
            <button type="button" className={styles.tabAdd} onClick={() => void addTable()} aria-label={strings.baseNewTable}>
              <Plus size={14} />
            </button>
          </div>

          {table && (
            <div className={styles.gridWrap}>
              <table className={styles.grid}>
                <thead>
                  <tr>
                    <th className={styles.rowNumHead}>#</th>
                    {table.fields.map((f) => (
                      <th key={f.id} className={styles.colHead}>
                        <span className={styles.colName}>{f.name}</span>
                        <span className={styles.colType}>{f.type}</span>
                      </th>
                    ))}
                    <th className={styles.addColHead}>
                      <div className={styles.addColWrap} ref={addWrapRef}>
                        <button
                          type="button"
                          className={styles.addCol}
                          onClick={() => setAddingField((v) => !v)}
                          aria-label={strings.baseAddField}
                        >
                          <Plus size={15} />
                        </button>
                        {addingField && (
                          <div className={styles.addColMenu}>
                            <input
                              className={styles.addColInput}
                              autoFocus
                              value={newFieldName}
                              placeholder={strings.baseFieldName}
                              onChange={(e) => setNewFieldName(e.target.value)}
                              onKeyDown={(e) => e.key === "Enter" && void addField()}
                            />
                            <select
                              className={styles.addColSelect}
                              value={newFieldType}
                              onChange={(e) => setNewFieldType(e.target.value as BaseFieldType)}
                            >
                              {EDITABLE_TYPES.map((t) => (
                                <option key={t.type} value={t.type}>
                                  {t.label()}
                                </option>
                              ))}
                            </select>
                            <button type="button" className={styles.addColBtn} onClick={() => void addField()}>
                              {strings.baseAddField}
                            </button>
                          </div>
                        )}
                      </div>
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {table.records.map((r, ri) => (
                    <tr key={r.id}>
                      <td className={styles.rowNum}>{ri + 1}</td>
                      {table.fields.map((f) => (
                        <td key={f.id} className={styles.cell}>
                          <Cell field={f} value={r.cells[f.id]} onCommit={(v) => void setCell(r, f.id, v)} />
                        </td>
                      ))}
                      <td className={styles.cellPad} />
                    </tr>
                  ))}
                </tbody>
              </table>
              <button type="button" className={styles.addRow} onClick={() => void addRow()}>
                <Plus size={15} /> {strings.baseNewRow}
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
}

/** One editable cell, rendered by the field's type. */
function Cell({
  field,
  value,
  onCommit,
}: {
  field: BaseFieldDto;
  value: unknown;
  onCommit: (value: unknown) => void;
}) {
  switch (field.type) {
    case "checkbox":
      return (
        <input
          type="checkbox"
          className={styles.check}
          checked={value === true}
          onChange={(e) => onCommit(e.target.checked)}
        />
      );
    case "number":
      return (
        <input
          type="number"
          className={styles.input}
          defaultValue={typeof value === "number" ? value : (value as string) ?? ""}
          onBlur={(e) => onCommit(e.target.value === "" ? null : Number(e.target.value))}
        />
      );
    case "date":
      return (
        <input
          type="date"
          className={styles.input}
          defaultValue={typeof value === "string" ? value : ""}
          onChange={(e) => onCommit(e.target.value || null)}
        />
      );
    default:
      // text (and, for now, any not-yet-editable type shown as text)
      return (
        <input
          type="text"
          className={styles.input}
          defaultValue={value === null || value === undefined ? "" : String(value)}
          onBlur={(e) => onCommit(e.target.value)}
        />
      );
  }
}
