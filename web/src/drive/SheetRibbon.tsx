// alo Sheet's own ribbon — our UI over Univer's open engine (ADR 0033). Univer's
// built-in toolbar is hidden (`toolbar: false`); this renders in its place and
// drives the engine through the `SheetActions` the editor passes in. Pure
// presentation: it holds no Univer types, so the engine coupling lives in one
// place (SheetEditor), per one-file-one-reason.
import { useState } from "react";
import {
  AArrowDown,
  AArrowUp,
  AlignCenter,
  AlignLeft,
  AlignRight,
  AlignVerticalJustifyCenter,
  AlignVerticalJustifyEnd,
  AlignVerticalJustifyStart,
  Baseline,
  Bold,
  Eraser,
  Italic,
  PaintBucket,
  Redo2,
  Snowflake,
  Strikethrough,
  TableCellsMerge,
  Underline,
  Undo2,
  WrapText,
} from "lucide-react";

import { strings } from "../i18n";
import styles from "./SheetRibbon.module.css";

/** What the ribbon can ask the engine to do. Implemented by SheetEditor against
 *  Univer's facade; the ribbon itself never touches Univer. */
export interface SheetActions {
  /** Run a Univer command by id (the toggle formats: bold/italic/…). */
  exec: (commandId: string) => void;
  setFontFamily: (family: string) => void;
  setFontSize: (size: number) => void;
  adjustFontSize: (delta: number) => void;
  setFontColor: (hex: string) => void;
  setFillColor: (hex: string) => void;
  align: (a: "left" | "center" | "right") => void;
  valign: (a: "top" | "middle" | "bottom") => void;
  toggleWrap: () => void;
  merge: () => void;
  setNumberFormat: (pattern: string) => void;
  insertRow: (where: "before" | "after") => void;
  insertColumn: (where: "before" | "after") => void;
  deleteRow: () => void;
  deleteColumn: () => void;
  clearContents: () => void;
  clearFormats: () => void;
  freezeAtSelection: () => void;
  unfreeze: () => void;
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

// Number-format presets (EU-friendly). Value is an Excel-style format pattern.
const NUMBER_FORMATS: { key: string; label: string; pattern: string }[] = [
  { key: "general", label: "General", pattern: "General" },
  { key: "number", label: "1,234.56", pattern: "#,##0.00" },
  { key: "currency", label: "€ 1,234.56", pattern: "€ #,##0.00" },
  { key: "percent", label: "12.34%", pattern: "0.00%" },
  { key: "date", label: "2026-08-06", pattern: "yyyy-mm-dd" },
  { key: "text", label: "Text", pattern: "@" },
];

const TABS = ["home", "insert", "draw", "layout", "formulas", "data", "review", "view"] as const;
type Tab = (typeof TABS)[number];
// Tabs whose tools need Univer plugins we haven't wired yet — honest placeholder.
const SOON: Tab[] = ["draw", "layout", "formulas", "data", "review"];

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

export function SheetRibbon({ actions, disabled }: { actions: SheetActions; disabled: boolean }) {
  const [tab, setTab] = useState<Tab>("home");

  return (
    <div className={styles.ribbon} role="toolbar" aria-label={strings.sheetRibbon}>
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

      {tab === "home" && <HomeTab actions={actions} disabled={disabled} />}
      {tab === "insert" && <InsertTab actions={actions} disabled={disabled} />}
      {tab === "view" && <ViewTab actions={actions} disabled={disabled} />}
      {SOON.includes(tab) && <div className={styles.soon}>{strings.sheetTabSoon(tabLabel(tab))}</div>}
    </div>
  );
}

function HomeTab({ actions, disabled }: { actions: SheetActions; disabled: boolean }) {
  return (
    <div className={styles.groups}>
      <Group label={strings.sheetGroupHistory}>
        <div className={styles.row}>
          <IconBtn label={strings.sheetUndo} onClick={actions.undo} disabled={disabled}>
            <Undo2 size={16} />
          </IconBtn>
          <IconBtn label={strings.sheetRedo} onClick={actions.redo} disabled={disabled}>
            <Redo2 size={16} />
          </IconBtn>
        </div>
      </Group>

      <Group label={strings.sheetGroupFont}>
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
            <IconBtn label={strings.sheetFontGrow} onClick={() => actions.adjustFontSize(1)} disabled={disabled}>
              <AArrowUp size={16} />
            </IconBtn>
            <IconBtn label={strings.sheetFontShrink} onClick={() => actions.adjustFontSize(-1)} disabled={disabled}>
              <AArrowDown size={16} />
            </IconBtn>
          </div>
          <div className={styles.row}>
            <IconBtn label={strings.sheetBold} onClick={() => actions.exec(CMD_BOLD)} disabled={disabled}>
              <Bold size={16} />
            </IconBtn>
            <IconBtn label={strings.sheetItalic} onClick={() => actions.exec(CMD_ITALIC)} disabled={disabled}>
              <Italic size={16} />
            </IconBtn>
            <IconBtn label={strings.sheetUnderline} onClick={() => actions.exec(CMD_UNDERLINE)} disabled={disabled}>
              <Underline size={16} />
            </IconBtn>
            <IconBtn label={strings.sheetStrike} onClick={() => actions.exec(CMD_STRIKE)} disabled={disabled}>
              <Strikethrough size={16} />
            </IconBtn>
            <ColorBtn label={strings.sheetFontColor} onPick={actions.setFontColor} disabled={disabled}>
              <Baseline size={16} />
            </ColorBtn>
            <ColorBtn label={strings.sheetFillColor} onPick={actions.setFillColor} disabled={disabled}>
              <PaintBucket size={16} />
            </ColorBtn>
          </div>
        </div>
      </Group>

      <Group label={strings.sheetGroupAlignment}>
        <div className={styles.rowStack}>
          <div className={styles.row}>
            <IconBtn label={strings.sheetAlignTop} onClick={() => actions.valign("top")} disabled={disabled}>
              <AlignVerticalJustifyStart size={16} />
            </IconBtn>
            <IconBtn label={strings.sheetAlignMiddle} onClick={() => actions.valign("middle")} disabled={disabled}>
              <AlignVerticalJustifyCenter size={16} />
            </IconBtn>
            <IconBtn label={strings.sheetAlignBottom} onClick={() => actions.valign("bottom")} disabled={disabled}>
              <AlignVerticalJustifyEnd size={16} />
            </IconBtn>
            <IconBtn label={strings.sheetWrap} onClick={actions.toggleWrap} disabled={disabled}>
              <WrapText size={16} />
            </IconBtn>
          </div>
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
        </div>
      </Group>

      <Group label={strings.sheetGroupNumber}>
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
      </Group>

      <Group label={strings.sheetGroupCells}>
        <div className={styles.rowStack}>
          <div className={styles.row}>
            <TextBtn label={strings.sheetInsertRowAbove} onClick={() => actions.insertRow("before")} disabled={disabled} />
            <TextBtn label={strings.sheetInsertColLeft} onClick={() => actions.insertColumn("before")} disabled={disabled} />
          </div>
          <div className={styles.row}>
            <TextBtn label={strings.sheetDeleteRow} onClick={actions.deleteRow} disabled={disabled} />
            <TextBtn label={strings.sheetDeleteColumn} onClick={actions.deleteColumn} disabled={disabled} />
          </div>
        </div>
      </Group>

      <Group label={strings.sheetGroupClear}>
        <div className={styles.row}>
          <IconBtn label={strings.sheetClearFormats} onClick={actions.clearFormats} disabled={disabled}>
            <Eraser size={16} />
          </IconBtn>
          <TextBtn label={strings.sheetClearContents} onClick={actions.clearContents} disabled={disabled} />
        </div>
      </Group>
    </div>
  );
}

function InsertTab({ actions, disabled }: { actions: SheetActions; disabled: boolean }) {
  return (
    <div className={styles.groups}>
      <Group label={strings.sheetGroupRows}>
        <div className={styles.rowStack}>
          <TextBtn label={strings.sheetInsertRowAbove} onClick={() => actions.insertRow("before")} disabled={disabled} />
          <TextBtn label={strings.sheetInsertRowBelow} onClick={() => actions.insertRow("after")} disabled={disabled} />
        </div>
      </Group>
      <Group label={strings.sheetGroupColumns}>
        <div className={styles.rowStack}>
          <TextBtn label={strings.sheetInsertColLeft} onClick={() => actions.insertColumn("before")} disabled={disabled} />
          <TextBtn label={strings.sheetInsertColRight} onClick={() => actions.insertColumn("after")} disabled={disabled} />
        </div>
      </Group>
    </div>
  );
}

function ViewTab({ actions, disabled }: { actions: SheetActions; disabled: boolean }) {
  return (
    <div className={styles.groups}>
      <Group label={strings.sheetGroupView}>
        <div className={styles.row}>
          <IconBtn label={strings.sheetFreeze} onClick={actions.freezeAtSelection} disabled={disabled}>
            <Snowflake size={16} />
          </IconBtn>
          <TextBtn label={strings.sheetUnfreeze} onClick={actions.unfreeze} disabled={disabled} />
        </div>
      </Group>
    </div>
  );
}

function Group({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className={styles.group}>
      {children}
      <span className={styles.groupLabel}>{label}</span>
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

function TextBtn({ label, onClick, disabled }: { label: string; onClick: () => void; disabled: boolean }) {
  return (
    <button type="button" className={styles.textBtn} onClick={onClick} disabled={disabled} title={label}>
      {label}
    </button>
  );
}

/** An icon button that opens the native colour picker and reports the chosen hex. */
function ColorBtn({
  label,
  onPick,
  disabled,
  children,
}: {
  label: string;
  onPick: (hex: string) => void;
  disabled: boolean;
  children: React.ReactNode;
}) {
  return (
    <label className={styles.colorBtn} title={label} aria-label={label}>
      {children}
      <input
        type="color"
        className={styles.colorInput}
        disabled={disabled}
        defaultValue="#000000"
        onChange={(e) => onPick(e.target.value)}
      />
    </label>
  );
}
