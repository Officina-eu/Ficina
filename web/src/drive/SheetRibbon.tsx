// alo Sheet's own ribbon — our UI over Univer's open engine (ADR 0033). Univer's
// built-in toolbar is hidden (`toolbar: false`); this renders in its place and
// drives the engine through the `SheetActions` the editor passes in. Pure
// presentation: it holds no Univer types, so the engine coupling lives in one
// place (SheetEditor), per one-file-one-reason.
import { useState } from "react";
import {
  AlignCenter,
  AlignLeft,
  AlignRight,
  Bold,
  Italic,
  Redo2,
  Strikethrough,
  TableCellsMerge,
  Underline,
  Undo2,
} from "lucide-react";

import { strings } from "../i18n";
import styles from "./SheetRibbon.module.css";

/** What the ribbon can ask the engine to do. Implemented by SheetEditor against
 *  Univer's facade; the ribbon itself never touches Univer. */
export interface SheetActions {
  /** Run a Univer command by id (used for the toggle formats: bold/italic/…). */
  exec: (commandId: string) => void;
  setFontFamily: (family: string) => void;
  setFontSize: (size: number) => void;
  align: (a: "left" | "center" | "right") => void;
  merge: () => void;
  setNumberFormat: (pattern: string) => void;
  undo: () => void;
  redo: () => void;
}

// Univer command ids for the toggle formats (verified against @univerjs/sheets).
const CMD_BOLD = "sheet.command.set-range-bold";
const CMD_ITALIC = "sheet.command.set-range-italic";
const CMD_UNDERLINE = "sheet.command.set-range-underline";
const CMD_STRIKE = "sheet.command.set-range-stroke";

const FONTS = ["Calibri", "Arial", "Times New Roman", "Georgia", "Verdana", "Courier New"];
const SIZES = [8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72];

/** The tabs shown in the strip. Only Home is built; the rest show an honest
 *  "coming soon" panel so the strip matches the design without faking tools. */
const TABS = [
  "home",
  "insert",
  "draw",
  "layout",
  "formulas",
  "data",
  "review",
  "view",
] as const;
type Tab = (typeof TABS)[number];

function tabLabel(tab: Tab): string {
  switch (tab) {
    case "home":
      return strings.sheetTabHome;
    case "insert":
      return strings.sheetTabInsert;
    case "draw":
      return strings.sheetTabDraw;
    case "layout":
      return strings.sheetTabLayout;
    case "formulas":
      return strings.sheetTabFormulas;
    case "data":
      return strings.sheetTabData;
    case "review":
      return strings.sheetTabReview;
    case "view":
      return strings.sheetTabView;
  }
}

// Number-format presets (EU-friendly). Value is an Excel-style format pattern.
const NUMBER_FORMATS: { key: string; label: string; pattern: string }[] = [
  { key: "general", label: "General", pattern: "General" },
  { key: "number", label: "1,234.56", pattern: "#,##0.00" },
  { key: "currency", label: "€ 1,234.56", pattern: "€ #,##0.00" },
  { key: "percent", label: "12.34%", pattern: "0.00%" },
  { key: "date", label: "2026-08-06", pattern: "yyyy-mm-dd" },
  { key: "text", label: "Text", pattern: "@" },
];

export function SheetRibbon({ actions, disabled }: { actions: SheetActions; disabled: boolean }) {
  const [tab, setTab] = useState<Tab>("home");

  return (
    <div className={styles.ribbon} role="toolbar" aria-label={strings.sheetRibbon}>
      {/* Tab strip. */}
      <div className={styles.tabs} role="tablist">
        {TABS.map((t) => (
          <button
            key={t}
            type="button"
            role="tab"
            aria-selected={t === tab}
            className={t === tab ? styles.tabActive : styles.tab}
            onClick={() => setTab(t)}
          >
            {tabLabel(t)}
          </button>
        ))}
      </div>

      {tab === "home" ? (
        <div className={styles.groups}>
          {/* Undo / Redo */}
          <div className={styles.group}>
            <div className={styles.row}>
              <IconBtn label={strings.sheetUndo} onClick={actions.undo} disabled={disabled}>
                <Undo2 size={16} />
              </IconBtn>
              <IconBtn label={strings.sheetRedo} onClick={actions.redo} disabled={disabled}>
                <Redo2 size={16} />
              </IconBtn>
            </div>
            <span className={styles.groupLabel}>{strings.sheetGroupHistory}</span>
          </div>

          {/* Font */}
          <div className={styles.group}>
            <div className={styles.rowStack}>
              <div className={styles.row}>
                <select
                  className={styles.fontSelect}
                  aria-label={strings.sheetFontFamily}
                  disabled={disabled}
                  defaultValue="Calibri"
                  onChange={(e) => actions.setFontFamily(e.target.value)}
                >
                  {FONTS.map((f) => (
                    <option key={f} value={f}>
                      {f}
                    </option>
                  ))}
                </select>
                <select
                  className={styles.sizeSelect}
                  aria-label={strings.sheetFontSize}
                  disabled={disabled}
                  defaultValue={11}
                  onChange={(e) => actions.setFontSize(Number(e.target.value))}
                >
                  {SIZES.map((s) => (
                    <option key={s} value={s}>
                      {s}
                    </option>
                  ))}
                </select>
              </div>
              <div className={styles.row}>
                <IconBtn label={strings.sheetBold} onClick={() => actions.exec(CMD_BOLD)} disabled={disabled}>
                  <Bold size={16} />
                </IconBtn>
                <IconBtn label={strings.sheetItalic} onClick={() => actions.exec(CMD_ITALIC)} disabled={disabled}>
                  <Italic size={16} />
                </IconBtn>
                <IconBtn
                  label={strings.sheetUnderline}
                  onClick={() => actions.exec(CMD_UNDERLINE)}
                  disabled={disabled}
                >
                  <Underline size={16} />
                </IconBtn>
                <IconBtn label={strings.sheetStrike} onClick={() => actions.exec(CMD_STRIKE)} disabled={disabled}>
                  <Strikethrough size={16} />
                </IconBtn>
              </div>
            </div>
            <span className={styles.groupLabel}>{strings.sheetGroupFont}</span>
          </div>

          {/* Alignment */}
          <div className={styles.group}>
            <div className={styles.row}>
              <IconBtn label={strings.sheetAlignLeft} onClick={() => actions.align("left")} disabled={disabled}>
                <AlignLeft size={16} />
              </IconBtn>
              <IconBtn label={strings.sheetAlignCenter} onClick={() => actions.align("center")} disabled={disabled}>
                <AlignCenter size={16} />
              </IconBtn>
              <IconBtn label={strings.sheetAlignRight} onClick={() => actions.align("right")} disabled={disabled}>
                <AlignRight size={16} />
              </IconBtn>
              <IconBtn label={strings.sheetMerge} onClick={actions.merge} disabled={disabled}>
                <TableCellsMerge size={16} />
              </IconBtn>
            </div>
            <span className={styles.groupLabel}>{strings.sheetGroupAlignment}</span>
          </div>

          {/* Number */}
          <div className={styles.group}>
            <div className={styles.row}>
              <select
                className={styles.numberSelect}
                aria-label={strings.sheetNumberFormat}
                disabled={disabled}
                defaultValue="general"
                onChange={(e) => {
                  const fmt = NUMBER_FORMATS.find((n) => n.key === e.target.value);
                  if (fmt) actions.setNumberFormat(fmt.pattern);
                }}
              >
                {NUMBER_FORMATS.map((n) => (
                  <option key={n.key} value={n.key}>
                    {n.label}
                  </option>
                ))}
              </select>
            </div>
            <span className={styles.groupLabel}>{strings.sheetGroupNumber}</span>
          </div>
        </div>
      ) : (
        <div className={styles.soon}>{strings.sheetTabSoon(tabLabel(tab))}</div>
      )}
    </div>
  );
}

function IconBtn({
  label,
  onClick,
  disabled,
  children,
}: {
  label: string;
  onClick: () => void;
  disabled: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      className={styles.iconBtn}
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      title={label}
    >
      {children}
    </button>
  );
}
