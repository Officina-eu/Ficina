// A rich-text editor for compose: a contentEditable surface with a Google-Docs-
// style toolbar (text styles, bold/italic/underline/strikethrough, text + highlight
// colour, lists, alignment, quote, rule, link, image, equation, code, clear).
// Formatting uses the browser's built-in editing commands; it emits HTML on every
// edit, and the parent derives a plain-text alternative from it.
import { Suspense, lazy, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import {
  AlignCenter,
  AlignLeft,
  AlignRight,
  Baseline,
  Bold,
  Code2,
  Eraser,
  Highlighter,
  Image as ImageIcon,
  Italic,
  Link2,
  List,
  ListOrdered,
  Minus,
  Quote,
  Sigma,
  Strikethrough,
  Underline,
} from "lucide-react";

import { strings } from "../../i18n";
import styles from "./RichTextEditor.module.css";

// The equation/code insert UI pulls in KaTeX + Prism, so it is code-split: those
// libraries load only when a user inserts one, never on the mail path (ADR 0015).
const AuthoringInsertModal = lazy(() =>
  import("../../authoring").then((m) => ({ default: m.AuthoringInsertModal })),
);

/** Largest inline image edge (px); wider images are downscaled before embedding. */
const MAX_IMAGE_EDGE = 1400;

interface RichTextEditorProps {
  /** Initial HTML (uncontrolled thereafter — set once on mount). */
  initialHtml: string;
  /** Called with the editor's current HTML on every edit. */
  onChange: (html: string) => void;
  placeholder: string;
  autoFocus?: boolean;
}

/** Load a data URL into an Image element. */
function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = reject;
    img.src = src;
  });
}

/** Read a file, downscaling large images so the embedded data URI stays sane. */
async function imageDataUrl(file: File): Promise<string> {
  const raw = await new Promise<string>((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result));
    r.onerror = reject;
    r.readAsDataURL(file);
  });
  try {
    const img = await loadImage(raw);
    const longest = Math.max(img.width, img.height);
    if (longest <= MAX_IMAGE_EDGE) return raw;
    const scale = MAX_IMAGE_EDGE / longest;
    const canvas = document.createElement("canvas");
    canvas.width = Math.round(img.width * scale);
    canvas.height = Math.round(img.height * scale);
    const ctx = canvas.getContext("2d");
    if (ctx === null) return raw;
    ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
    const type = file.type === "image/png" ? "image/png" : "image/jpeg";
    return canvas.toDataURL(type, 0.85);
  } catch {
    return raw;
  }
}

export function RichTextEditor({ initialHtml, onChange, placeholder, autoFocus }: RichTextEditorProps) {
  const ref = useRef<HTMLDivElement>(null);
  const savedRange = useRef<Range | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const [insert, setInsert] = useState<null | "equation" | "code">(null);

  useEffect(() => {
    const el = ref.current;
    if (el === null) return;
    el.innerHTML = initialHtml;
    if (autoFocus === true) el.focus();
  }, [initialHtml, autoFocus]);

  function emit() {
    onChange(ref.current?.innerHTML ?? "");
  }

  /** Remember the caret so a control that steals focus (colour picker, file
   * dialog, insert modal) can restore where the user was. */
  function saveRange() {
    const sel = window.getSelection();
    if (
      sel !== null &&
      sel.rangeCount > 0 &&
      ref.current?.contains(sel.getRangeAt(0).commonAncestorContainer) === true
    ) {
      savedRange.current = sel.getRangeAt(0).cloneRange();
    }
  }

  function restoreRange() {
    const el = ref.current;
    if (el === null) return;
    el.focus();
    const sel = window.getSelection();
    if (sel === null || savedRange.current === null) return;
    sel.removeAllRanges();
    sel.addRange(savedRange.current);
  }

  /** Run an editing command with the selection intact (toolbar buttons keep it
   * via preventDefault on mousedown). */
  function exec(command: string, value?: string) {
    ref.current?.focus();
    document.execCommand(command, false, value);
    emit();
  }

  /** Run a command that needs the pre-blur selection restored first. */
  function execRestored(command: string, value: string) {
    restoreRange();
    document.execCommand(command, false, value);
    emit();
  }

  function addLink() {
    saveRange();
    const url = window.prompt(strings.linkPrompt);
    if (url === null || url.trim().length === 0) return;
    execRestored("createLink", url.trim());
  }

  function openInsert(kind: "equation" | "code") {
    saveRange();
    setInsert(kind);
  }

  /** Insert HTML at the saved caret, parsed via <template> so MathML/atoms survive. */
  function insertHtml(html: string) {
    setInsert(null);
    const el = ref.current;
    if (el === null) return;
    el.focus();
    const sel = window.getSelection();
    if (sel === null) return;
    let range: Range;
    if (savedRange.current !== null) {
      range = savedRange.current;
    } else {
      range = document.createRange();
      range.selectNodeContents(el);
      range.collapse(false);
    }
    range.deleteContents();
    const tpl = document.createElement("template");
    tpl.innerHTML = `${html}&nbsp;`;
    const lastNode = tpl.content.lastChild;
    range.insertNode(tpl.content);
    if (lastNode !== null) {
      const after = document.createRange();
      after.setStartAfter(lastNode);
      after.collapse(true);
      sel.removeAllRanges();
      sel.addRange(after);
    }
    emit();
  }

  async function onPickImage(file: File) {
    if (!file.type.startsWith("image/")) return;
    const dataUrl = await imageDataUrl(file);
    insertHtml(`<img src="${dataUrl}" alt="" style="max-width:100%;height:auto" />`);
  }

  /** A toolbar button (keeps the editor selection via mousedown preventDefault). */
  function tool(key: string, label: string, icon: ReactNode, onClick: () => void) {
    return (
      <button
        key={key}
        type="button"
        className={styles.tool}
        aria-label={label}
        title={label}
        onMouseDown={(e) => e.preventDefault()}
        onClick={onClick}
      >
        {icon}
      </button>
    );
  }

  const divider = (k: string) => <span key={k} className={styles.divider} aria-hidden="true" />;

  return (
    <div className={styles.wrap}>
      <div className={styles.toolbar} role="toolbar" aria-label={strings.formatting}>
        <select
          className={styles.select}
          aria-label={strings.textStyle}
          defaultValue=""
          onChange={(e) => {
            const v = e.target.value;
            e.currentTarget.selectedIndex = 0;
            if (v !== "") exec("formatBlock", v);
          }}
        >
          <option value="">{strings.styleNormal}</option>
          <option value="h1">{strings.styleHeading}</option>
          <option value="h2">{strings.styleSubheading}</option>
          <option value="blockquote">{strings.styleQuote}</option>
        </select>
        {divider("d0")}

        {tool("bold", strings.bold, <Bold size={16} />, () => exec("bold"))}
        {tool("italic", strings.italic, <Italic size={16} />, () => exec("italic"))}
        {tool("underline", strings.underline, <Underline size={16} />, () => exec("underline"))}
        {tool("strike", strings.strikethrough, <Strikethrough size={16} />, () =>
          exec("strikeThrough"),
        )}

        <label className={styles.color} title={strings.textColor}>
          <Baseline size={16} />
          <input
            type="color"
            className={styles.colorInput}
            onMouseDown={saveRange}
            onChange={(e) => execRestored("foreColor", e.target.value)}
          />
        </label>
        <label className={styles.color} title={strings.highlight}>
          <Highlighter size={16} />
          <input
            type="color"
            className={styles.colorInput}
            defaultValue="#fff2a8"
            onMouseDown={saveRange}
            onChange={(e) => execRestored("hiliteColor", e.target.value)}
          />
        </label>
        {divider("d1")}

        {tool("ul", strings.bulletList, <List size={16} />, () => exec("insertUnorderedList"))}
        {tool("ol", strings.numberedList, <ListOrdered size={16} />, () =>
          exec("insertOrderedList"),
        )}
        {tool("alignL", strings.alignLeft, <AlignLeft size={16} />, () => exec("justifyLeft"))}
        {tool("alignC", strings.alignCenter, <AlignCenter size={16} />, () => exec("justifyCenter"))}
        {tool("alignR", strings.alignRight, <AlignRight size={16} />, () => exec("justifyRight"))}
        {divider("d2")}

        {tool("quote", strings.styleQuote, <Quote size={16} />, () =>
          exec("formatBlock", "blockquote"),
        )}
        {tool("hr", strings.horizontalRule, <Minus size={16} />, () => exec("insertHorizontalRule"))}
        {tool("link", strings.link, <Link2 size={16} />, addLink)}
        {tool("image", strings.insertImage, <ImageIcon size={16} />, () => {
          saveRange();
          fileInput.current?.click();
        })}
        {divider("d3")}

        {tool("eq", strings.composeInsertEquation, <Sigma size={16} />, () => openInsert("equation"))}
        {tool("code", strings.composeInsertCode, <Code2 size={16} />, () => openInsert("code"))}
        {tool("clear", strings.clearFormatting, <Eraser size={16} />, () => exec("removeFormat"))}
      </div>

      <input
        ref={fileInput}
        type="file"
        accept="image/*"
        className={styles.fileInput}
        onChange={(e) => {
          const f = e.target.files?.[0];
          if (f !== undefined) void onPickImage(f);
          e.target.value = "";
        }}
      />

      <div
        ref={ref}
        className={styles.editor}
        contentEditable
        role="textbox"
        aria-multiline="true"
        aria-label={placeholder}
        data-placeholder={placeholder}
        onInput={emit}
        onBlur={saveRange}
        suppressContentEditableWarning
      />
      {insert !== null && (
        <Suspense fallback={null}>
          <AuthoringInsertModal kind={insert} onInsert={insertHtml} onClose={() => setInsert(null)} />
        </Suspense>
      )}
    </div>
  );
}
