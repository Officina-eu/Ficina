// The block editor (ADR 0015): renders a document's blocks in order, each with
// reorder / delete / insert controls, and keeps the cross-reference numbering
// live off the current block order. Fully controlled — the parent owns the
// blocks and persists them (autosave).
import { useMemo, useState } from "react";
import { ChevronDown, ChevronUp, Code2, Heading, Plus, Sigma, Table2, Text, Trash2 } from "lucide-react";

import { strings } from "../i18n";
import { cx } from "../ds";
import { type Block, type BlockType, blankBlock, blocksToItems } from "./document";
import { computeNumbering } from "./numbering";
import { HeadingBlock } from "./HeadingBlock";
import { ParagraphBlock } from "./ParagraphBlock";
import { EquationBlock } from "./EquationBlock";
import { CodeBlock } from "./CodeBlock";
import { TableBlock } from "./TableBlock";
import styles from "./DocumentEditor.module.css";

const ADD_ITEMS: { type: BlockType; label: string; Icon: typeof Text }[] = [
  { type: "heading", label: "Heading", Icon: Heading },
  { type: "paragraph", label: "Text", Icon: Text },
  { type: "equation", label: "Equation", Icon: Sigma },
  { type: "code", label: "Code block", Icon: Code2 },
  { type: "table", label: "Table", Icon: Table2 },
];

/** The "+ add block" control shown after each block. */
function AddBlock({ onAdd }: { onAdd: (type: BlockType) => void }) {
  const [open, setOpen] = useState(false);
  return (
    <div className={styles.addRow}>
      <button
        type="button"
        className={cx(styles.addBtn, open && styles.addBtnOn)}
        onClick={() => setOpen((v) => !v)}
        aria-label={strings.blockAdd}
      >
        <Plus size={15} />
      </button>
      {open && (
        <div className={styles.addMenu}>
          {ADD_ITEMS.map(({ type, label, Icon }) => (
            <button
              key={type}
              type="button"
              className={styles.addMenuItem}
              onClick={() => {
                onAdd(type);
                setOpen(false);
              }}
            >
              <Icon size={15} />
              {label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

interface DocumentEditorProps {
  blocks: Block[];
  onChange: (blocks: Block[]) => void;
}

export function DocumentEditor({ blocks, onChange }: DocumentEditorProps) {
  const items = useMemo(() => blocksToItems(blocks), [blocks]);
  const numbering = useMemo(() => computeNumbering(items), [items]);

  function patch(id: string, next: Partial<Block>) {
    onChange(blocks.map((b) => (b.id === id ? ({ ...b, ...next } as Block) : b)));
  }

  function move(index: number, delta: number) {
    const target = index + delta;
    if (target < 0 || target >= blocks.length) return;
    const next = [...blocks];
    const a = next[index];
    const b = next[target];
    if (a === undefined || b === undefined) return;
    next[index] = b;
    next[target] = a;
    onChange(next);
  }

  function remove(id: string) {
    onChange(blocks.filter((b) => b.id !== id));
  }

  function addAfter(index: number, type: BlockType) {
    const next = [...blocks];
    next.splice(index + 1, 0, blankBlock(type));
    onChange(next);
  }

  function renderBlock(block: Block) {
    switch (block.type) {
      case "heading":
        return (
          <HeadingBlock
            level={block.level}
            text={block.text}
            number={numbering.get(block.id)?.display}
            onChange={(text) => patch(block.id, { text })}
            onLevelChange={(level) => patch(block.id, { level })}
          />
        );
      case "paragraph":
        return (
          <ParagraphBlock
            text={block.text}
            items={items}
            numbering={numbering}
            onChange={(text) => patch(block.id, { text })}
          />
        );
      case "equation":
        return (
          <EquationBlock
            latex={block.latex}
            numbered={block.numbered}
            number={numbering.get(block.id)?.display}
            onChange={(latex) => patch(block.id, { latex })}
            onToggleNumbered={(numbered) => patch(block.id, { numbered })}
          />
        );
      case "code":
        return (
          <CodeBlock
            code={block.code}
            language={block.language}
            onChange={(code) => patch(block.id, { code })}
            onLanguageChange={(language) => patch(block.id, { language })}
          />
        );
      case "table":
        return (
          <TableBlock
            rows={block.rows}
            number={numbering.get(block.id)?.display}
            onChange={(rows) => patch(block.id, { rows })}
          />
        );
    }
  }

  return (
    <div className={styles.blocks}>
      {blocks.map((block, index) => (
        <div key={block.id} className={styles.blockRow}>
          <div className={styles.controls}>
            <button
              type="button"
              aria-label={strings.blockMoveUp}
              disabled={index === 0}
              onClick={() => move(index, -1)}
            >
              <ChevronUp size={15} />
            </button>
            <button
              type="button"
              aria-label={strings.blockMoveDown}
              disabled={index === blocks.length - 1}
              onClick={() => move(index, 1)}
            >
              <ChevronDown size={15} />
            </button>
            <button
              type="button"
              className={styles.delete}
              aria-label={strings.blockDelete}
              onClick={() => remove(block.id)}
            >
              <Trash2 size={15} />
            </button>
          </div>
          <div className={styles.blockBody}>{renderBlock(block)}</div>
          <AddBlock onAdd={(type) => addAfter(index, type)} />
        </div>
      ))}
      {blocks.length === 0 && (
        <div className={styles.emptyAdd}>
          <AddBlock onAdd={(type) => addAfter(-1, type)} />
          <span className={styles.emptyHint}>{strings.blockEmptyHint}</span>
        </div>
      )}
    </div>
  );
}
