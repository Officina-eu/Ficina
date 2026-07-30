// The technical-authoring workspace (ADR 0015): the equation editor, code block,
// and cross-reference/auto-numbering engine composed into one surface. This is
// the standalone Ficina Docs surface that renders these tools today; when the
// Collabora Docs shell lands, the same components dock into it. Everything here
// renders in the browser — no draft equation or line of code leaves the client.
import { useMemo, useState } from "react";
import { ChevronDown, ChevronUp, Link2, Plus } from "lucide-react";

import { strings } from "../i18n";
import { cx } from "../ds";
import { type DocItem, computeNumbering, referenceText } from "./numbering";
import { EquationEditor } from "./EquationEditor";
import { CodeBlock } from "./CodeBlock";
import { CrossReferencePicker, ReferenceChip, refLabels } from "./CrossReference";
import styles from "./AuthoringWorkspace.module.css";

const SAMPLE_CODE = `// Relativistic kinetic energy, evaluated numerically.
export function kineticEnergy(mass: number, velocity: number): number {
  const c = 299_792_458; // speed of light, m/s
  const lorentz = 1 / Math.sqrt(1 - (velocity / c) ** 2);
  return mass * c ** 2 * (lorentz - 1);
}`;

const INITIAL_ITEMS: DocItem[] = [
  { id: "sec:overview", kind: "section", level: 1, title: "Overview" },
  { id: "eq:energy", kind: "equation", title: "Mass–energy equivalence" },
  { id: "sec:derivation", kind: "section", level: 2, title: "Derivation" },
  { id: "eq:wave", kind: "equation", title: "Wave equation" },
  { id: "tab:results", kind: "table", title: "Benchmark results" },
  { id: "fig:arch", kind: "figure", title: "System architecture" },
];

export function AuthoringWorkspace() {
  const [items, setItems] = useState<DocItem[]>(INITIAL_ITEMS);
  const numbering = useMemo(() => computeNumbering(items), [items]);

  const [energyLatex, setEnergyLatex] = useState("E = mc^2");
  const [energyDisplay, setEnergyDisplay] = useState(true);
  const [energyNumbered, setEnergyNumbered] = useState(true);

  const [code, setCode] = useState(SAMPLE_CODE);
  const [language, setLanguage] = useState("typescript");

  const [refs, setRefs] = useState<string[]>(["eq:energy", "tab:results"]);
  const [pickerOpen, setPickerOpen] = useState(false);

  const energyNumber = numbering.get("eq:energy")?.display;

  function move(index: number, delta: number) {
    setItems((prev) => {
      const target = index + delta;
      if (target < 0 || target >= prev.length) return prev;
      const next = [...prev];
      const a = next[index];
      const b = next[target];
      if (a === undefined || b === undefined) return prev;
      next[index] = b;
      next[target] = a;
      return next;
    });
  }

  return (
    <div className={styles.workspace}>
      <header className={styles.header}>
        <h1 className={styles.title}>{strings.authoringTitle}</h1>
        <p className={styles.subtitle}>{strings.authoringSubtitle}</p>
      </header>

      {/* Equations */}
      <section className={styles.section}>
        <h2 className={styles.sectionTitle}>{strings.authoringEquations}</h2>
        <p className={styles.sectionHint}>{strings.authoringEquationsHint}</p>
        <EquationEditor
          value={energyLatex}
          onChange={setEnergyLatex}
          display={energyDisplay}
          onDisplayChange={setEnergyDisplay}
          numbered={energyNumbered}
          onNumberedChange={setEnergyNumbered}
          number={energyNumber}
        />
      </section>

      {/* Code */}
      <section className={styles.section}>
        <h2 className={styles.sectionTitle}>{strings.authoringCode}</h2>
        <p className={styles.sectionHint}>{strings.authoringCodeHint}</p>
        <CodeBlock
          code={code}
          onChange={setCode}
          language={language}
          onLanguageChange={setLanguage}
          filename="energy.ts"
        />
      </section>

      {/* Cross-references + auto-numbering */}
      <section className={styles.section}>
        <h2 className={styles.sectionTitle}>{strings.authoringCrossRefs}</h2>
        <p className={styles.sectionHint}>{strings.authoringCrossRefsHint}</p>

        <div className={styles.refSentence}>
          {strings.authoringRefLead}{" "}
          {refs.map((id, i) => (
            <span key={`${id}-${i}`}>
              <ReferenceChip targetId={id} numbering={numbering} />
              {i < refs.length - 1 ? ", " : " "}
            </span>
          ))}
          {strings.authoringRefTail}
        </div>

        <div className={styles.refActions}>
          <div className={styles.pickerAnchor}>
            <button
              type="button"
              className={styles.insertBtn}
              onClick={() => setPickerOpen((v) => !v)}
            >
              <Link2 size={15} />
              {strings.refInsert}
            </button>
            {pickerOpen && (
              <div className={styles.pickerPop}>
                <CrossReferencePicker
                  items={items}
                  numbering={numbering}
                  onClose={() => setPickerOpen(false)}
                  onPick={(id) => {
                    setRefs((prev) => [...prev, id]);
                    setPickerOpen(false);
                  }}
                />
              </div>
            )}
          </div>
          {refs.length > 0 && (
            <button type="button" className={styles.clearBtn} onClick={() => setRefs([])}>
              {strings.authoringClearRefs}
            </button>
          )}
        </div>

        {/* The document outline — reorder any item and every number + chip above
            updates automatically, because references point at identities. */}
        <div className={styles.outline}>
          <div className={styles.outlineHead}>
            <Plus size={14} className={styles.outlineIcon} />
            {strings.authoringOutline}
          </div>
          <ul className={styles.outlineList}>
            {items.map((item, i) => {
              const info = numbering.get(item.id);
              return (
                <li key={item.id} className={cx(styles.outlineItem, styles[`kind_${item.kind}`])}>
                  <span className={styles.outlineNumber}>
                    {info !== undefined ? referenceText(info, refLabels()) : "—"}
                  </span>
                  <span className={styles.outlineTitle}>{item.title}</span>
                  <div className={styles.outlineMove}>
                    <button
                      type="button"
                      aria-label={strings.authoringMoveUp}
                      disabled={i === 0}
                      onClick={() => move(i, -1)}
                    >
                      <ChevronUp size={15} />
                    </button>
                    <button
                      type="button"
                      aria-label={strings.authoringMoveDown}
                      disabled={i === items.length - 1}
                      onClick={() => move(i, 1)}
                    >
                      <ChevronDown size={15} />
                    </button>
                  </div>
                </li>
              );
            })}
          </ul>
        </div>
      </section>
    </div>
  );
}
