// The Ficina Docs technical-authoring surface (ADR 0015), matching the Figma
// Docs screens: a document top bar, the editor chrome (menu + formatting bar —
// the frame for the Collabora editor, ADR 0010), and a paper canvas holding a
// real spec. The math, code, and cross-reference tools inside are fully
// functional and render browser-local; the general word-processor chrome is the
// visual frame until Collabora is embedded.
import { useMemo, useState } from "react";
import {
  Bold,
  ChevronDown,
  Image as ImageIcon,
  Italic,
  Link as LinkIcon,
  List,
  ListOrdered,
  MessageSquare,
  Redo2,
  Sigma,
  Sparkles,
  Table as TableIcon,
  Underline,
  Undo2,
} from "lucide-react";

import { strings } from "../i18n";
import { cx } from "../ds";
import { type DocItem, computeNumbering } from "./numbering";
import { renderMath } from "./katex";
import { EquationEditor } from "./EquationEditor";
import { CodeBlock } from "./CodeBlock";
import { CrossReferencePicker, ReferenceChip } from "./CrossReference";
import styles from "./AuthoringWorkspace.module.css";

const SAMPLE_CODE = `def heat_flux(k, dT, r1, r2):
    import math
    Q = 2 * math.pi * k * dT
    return Q / math.log(r2 / r1)

# measured: k=0.19 W/mK
flux = heat_flux(0.19, 42.0, 12e-3, 18e-3)`;

/** A rendered math span (inline or display). */
function Math({ latex, display }: { latex: string; display: boolean }) {
  const r = useMemo(() => renderMath(latex, display), [latex, display]);
  if (r.error !== null) return <span className={styles.mathError}>{latex}</span>;
  return <span dangerouslySetInnerHTML={{ __html: r.html }} />;
}

export function AuthoringWorkspace() {
  const [eqQLatex, setEqQLatex] = useState("Q = 2\\pi k L (T_1 - T_2)/\\ln(r_2/r_1)");
  const [fluxLatex] = useState("q = -k\\nabla T");
  const [code, setCode] = useState(SAMPLE_CODE);
  const [language, setLanguage] = useState("python");

  const [editingEq, setEditingEq] = useState(false);
  const [eqDraft, setEqDraft] = useState(eqQLatex);
  const [refPickerOpen, setRefPickerOpen] = useState(false);
  const [insertMenuOpen, setInsertMenuOpen] = useState(false);
  const [extraRefs, setExtraRefs] = useState<string[]>([]);

  // The document's numbered items. Equations carry their LaTeX so the
  // cross-reference picker can preview them (Figma).
  const items: DocItem[] = useMemo(
    () => [
      { id: "eq:flux", kind: "equation", title: "Fourier's law", latex: fluxLatex },
      { id: "eq:cont", kind: "equation", title: "Continuity", latex: "\\nabla \\cdot q = 0" },
      { id: "eq:Q", kind: "equation", title: "Radial conduction", latex: eqQLatex },
      {
        id: "eq:R",
        kind: "equation",
        title: "Thermal resistance",
        latex: "R = \\ln(r_2/r_1)/2\\pi k L",
      },
      { id: "sec:bc", kind: "section", level: 1, title: "Boundary conditions" },
      { id: "tab:cond", kind: "table", title: "Measured values" },
      { id: "fig:panel", kind: "figure", title: "Panel geometry" },
    ],
    [fluxLatex, eqQLatex],
  );
  const numbering = useMemo(() => computeNumbering(items), [items]);
  const eqQNumber = numbering.get("eq:Q")?.display;

  function openEquation() {
    setEqDraft(eqQLatex);
    setEditingEq(true);
    setInsertMenuOpen(false);
  }

  return (
    <div className={styles.app}>
      {/* Document top bar */}
      <div className={styles.docBar}>
        <div className={styles.docIcon}>W</div>
        <div className={styles.docMeta}>
          <div className={styles.docTitle}>{strings.docTitle}</div>
          <div className={styles.docSaved}>{strings.docSaved}</div>
        </div>
        <div className={styles.spacer} />
        <div className={styles.avatars}>
          <span className={cx(styles.avatar, styles.avatarK)}>K</span>
          <span className={cx(styles.avatar, styles.avatarH)}>H</span>
        </div>
        <button type="button" className={styles.askAi}>
          <Sparkles size={15} />
          {strings.docAskAi}
        </button>
        <button type="button" className={styles.share}>
          {strings.docShare}
        </button>
      </div>

      {/* Menu bar — the editor frame; Insert is wired to the authoring tools. */}
      <div className={styles.menuBar}>
        {["File", "Edit", "View"].map((m) => (
          <span key={m} className={styles.menuItem}>
            {m}
          </span>
        ))}
        <div className={styles.insertAnchor}>
          <button
            type="button"
            className={cx(styles.menuItem, styles.menuInsert)}
            onClick={() => setInsertMenuOpen((v) => !v)}
          >
            {strings.docInsert}
          </button>
          {insertMenuOpen && (
            <div className={styles.insertMenu} role="menu">
              <button type="button" className={styles.insertOption} onClick={openEquation}>
                <Sigma size={15} />
                {strings.insertEquation}
              </button>
              <button
                type="button"
                className={styles.insertOption}
                onClick={() => {
                  setRefPickerOpen(true);
                  setInsertMenuOpen(false);
                }}
              >
                <LinkIcon size={15} />
                {strings.insertCrossRef}
              </button>
            </div>
          )}
        </div>
        {["Format", "Tools", "Help"].map((m) => (
          <span key={m} className={styles.menuItem}>
            {m}
          </span>
        ))}
      </div>

      {/* Formatting toolbar — visual frame for the Collabora editor (ADR 0010). */}
      <div className={styles.toolbar} aria-hidden="true">
        <span className={styles.tbGroup}>
          <Undo2 size={16} />
          <Redo2 size={16} />
        </span>
        <span className={styles.tbDivider} />
        <span className={styles.tbSelect}>
          {strings.tbNormalText}
          <ChevronDown size={13} />
        </span>
        <span className={styles.tbSelect}>
          Inter
          <ChevronDown size={13} />
        </span>
        <span className={styles.tbDivider} />
        <span className={styles.tbGroup}>
          <Bold size={16} />
          <Italic size={16} />
          <Underline size={16} />
        </span>
        <span className={styles.tbDivider} />
        <span className={styles.tbGroup}>
          <LinkIcon size={16} />
          <MessageSquare size={16} />
          <ImageIcon size={16} />
          <TableIcon size={16} />
        </span>
        <span className={styles.tbDivider} />
        <span className={styles.tbGroup}>
          <List size={16} />
          <ListOrdered size={16} />
        </span>
        <span className={styles.spacer} />
        <span className={styles.tbEditing}>{strings.tbEditing}</span>
      </div>

      {/* The document canvas */}
      <div className={styles.canvas}>
        <article className={styles.page}>
          <h1 className={styles.h1}>{strings.specTitle}</h1>
          <p className={styles.docSubtitle}>{strings.specSubtitle}</p>

          <p className={styles.para}>
            {strings.specLead1}{" "}
            <span className={styles.inlineMath}>
              <Math latex={fluxLatex} display={false} />
            </span>{" "}
            {strings.specLead2}
          </p>

          {/* Numbered display equation — click to edit in the modal. */}
          <button type="button" className={styles.displayEq} onClick={openEquation}>
            <span className={styles.displayEqMath}>
              <Math latex={eqQLatex} display={true} />
            </span>
            {eqQNumber !== undefined && <span className={styles.eqNumber}>{`(${eqQNumber})`}</span>}
          </button>

          <p className={styles.para}>{strings.specMid}</p>

          <CodeBlock
            code={code}
            onChange={setCode}
            language={language}
            onLanguageChange={setLanguage}
          />

          <h2 className={styles.h2}>{strings.specBcHeading}</h2>
          <p className={styles.para}>
            {strings.specRefLead}{" "}
            <ReferenceChip targetId="eq:Q" numbering={numbering} />{" "}
            {strings.specRefMid}{" "}
            <ReferenceChip targetId="tab:cond" numbering={numbering} />
            {extraRefs.map((id) => (
              <span key={id}>
                {", "}
                <ReferenceChip targetId={id} numbering={numbering} />
              </span>
            ))}{" "}
            {strings.specRefTail}
          </p>

          <table className={styles.table}>
            <thead>
              <tr>
                <th>{strings.tblSymbol}</th>
                <th>{strings.tblValue}</th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td>k</td>
                <td>0.19 W/mK</td>
              </tr>
              <tr>
                <td>
                  r<sub>1</sub>
                </td>
                <td>12 mm</td>
              </tr>
              <tr>
                <td>
                  r<sub>2</sub>
                </td>
                <td>18 mm</td>
              </tr>
            </tbody>
          </table>

          <div className={styles.insertRefRow}>
            <button
              type="button"
              className={styles.insertRefBtn}
              onClick={() => setRefPickerOpen((v) => !v)}
            >
              <LinkIcon size={15} />
              {strings.refInsert}
            </button>
            {refPickerOpen && (
              <div className={styles.refPickerPop}>
                <CrossReferencePicker
                  items={items}
                  numbering={numbering}
                  onPick={(id) => {
                    setExtraRefs((prev) => (prev.includes(id) ? prev : [...prev, id]));
                    setRefPickerOpen(false);
                  }}
                />
              </div>
            )}
          </div>
        </article>
      </div>

      {editingEq && (
        <EquationEditor
          value={eqDraft}
          onChange={setEqDraft}
          display={true}
          onInsert={() => {
            setEqQLatex(eqDraft);
            setEditingEq(false);
          }}
          onClose={() => setEditingEq(false)}
        />
      )}
    </div>
  );
}
