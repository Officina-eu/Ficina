// Email-safe renderings of math and code (ADR 0015). Outgoing mail can't rely on
// our KaTeX/Prism CSS or fonts in the recipient's client, so:
//   - equations are emitted as **MathML** (renders natively in browsers,
//     including alo's reading-pane iframe; the LaTeX rides in `data-alo-latex`
//     and the message's text/plain part, so non-MathML clients still get it);
//   - code blocks are a **dark <pre> with fully inline styles** (Prism token
//     colours baked in), which renders everywhere with no external CSS.
// Both carry a data attribute so the plain-text derivation can reconstruct the
// LaTeX / fenced code, and are `contenteditable="false"` so the compose editor
// treats them as atomic.
import katex from "katex";

// `./prism` registers the language grammars as a side effect and exposes highlight.
import { highlight } from "./prism";

/** Escape a string for use inside a double-quoted HTML attribute. */
function escapeAttr(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** Dark-theme inline styles for Prism token classes (self-contained for email). */
const TOKEN_STYLE: Record<string, string> = {
  comment: "color:#6f7d74;font-style:italic",
  prolog: "color:#6f7d74;font-style:italic",
  doctype: "color:#6f7d74;font-style:italic",
  cdata: "color:#6f7d74;font-style:italic",
  punctuation: "color:#a9b3ac",
  keyword: "color:#d98a5c",
  atrule: "color:#d98a5c",
  operator: "color:#d98a5c",
  important: "color:#d98a5c",
  string: "color:#9bb07f",
  char: "color:#9bb07f",
  regex: "color:#9bb07f",
  url: "color:#9bb07f",
  "attr-value": "color:#9bb07f",
  number: "color:#d7b56b",
  boolean: "color:#d7b56b",
  constant: "color:#d7b56b",
  symbol: "color:#d7b56b",
  function: "color:#7fb0a3",
  "class-name": "color:#7fb0a3",
  tag: "color:#7fb0a3",
  property: "color:#7fb0a3",
  "attr-name": "color:#7fb0a3",
  builtin: "color:#7fb0a3",
};

/** Highlight code and bake Prism token colours in as inline styles. */
function inlineHighlight(code: string, language: string): string {
  const html = highlight(code, language);
  // Prism emits `<span class="token TYPE ...">`; swap the class for an inline style.
  return html.replace(/<span class="token ([a-z-]+)[^"]*">/g, (_m, type: string) => {
    const style = TOKEN_STYLE[type];
    return style !== undefined ? `<span style="${style}">` : "<span>";
  });
}

const PRE_STYLE =
  "background:#1b241f;color:#dcd8cf;padding:14px 18px;border-radius:12px;" +
  "overflow-x:auto;font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;" +
  "font-size:13px;line-height:1.7;white-space:pre;margin:12px 0";

/** Email HTML for a code block: a self-contained dark, inline-styled `<pre>`. */
export function codeEmailHtml(code: string, language: string): string {
  return (
    `<pre data-alo-lang="${escapeAttr(language)}" contenteditable="false" style="${PRE_STYLE}">` +
    `<code>${inlineHighlight(code, language)}</code></pre>`
  );
}

/** Email HTML for an equation: bare MathML (renders natively), with the LaTeX in
 * `data-alo-latex` for the plain-text fallback. `display` centers it. */
export function equationEmailHtml(latex: string, display: boolean): string {
  const rendered = katex.renderToString(latex, {
    output: "mathml",
    displayMode: display,
    throwOnError: false,
    trust: false,
  });
  // Keep only the <math> element (drop any KaTeX wrapper span, which has no
  // styling in an email anyway).
  const mathml = rendered.match(/<math[\s\S]*?<\/math>/i)?.[0] ?? rendered;
  const attr = escapeAttr(latex);
  if (display) {
    return `<div data-alo-latex="${attr}" contenteditable="false" style="text-align:center;margin:12px 0">${mathml}</div>`;
  }
  return `<span data-alo-latex="${attr}" contenteditable="false">${mathml}</span>`;
}
