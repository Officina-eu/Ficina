// KaTeX math rendering (ADR 0015). Renders LaTeX math to HTML entirely in the
// browser — the LaTeX never leaves the client. KaTeX ships its own fonts, so
// there is no CDN or external call. `renderMath` never throws: invalid LaTeX
// comes back as a structured error the editor shows inline, so a typo in a
// formula can never crash the surrounding document.
//
// The single `renderMath` seam is also the swap point for MathJax (ADR 0015's
// documented fallback) — same LaTeX input, so adopting it later touches only
// this file.
import katex from "katex";
import "katex/dist/katex.min.css";

export interface MathResult {
  /** Rendered KaTeX HTML, safe to inject (KaTeX escapes its input). Empty on error. */
  html: string;
  /** A human-readable message when the LaTeX is invalid, else null. */
  error: string | null;
}

/**
 * Render LaTeX math to HTML. `display` true renders a centered display equation,
 * false renders inline math sized to the surrounding text.
 */
export function renderMath(latex: string, display: boolean): MathResult {
  try {
    const html = katex.renderToString(latex, {
      displayMode: display,
      throwOnError: true,
      // Reject `\href`, `\includegraphics`, etc. — no untrusted commands may run
      // from a document's math.
      trust: false,
      strict: "ignore",
      output: "html",
    });
    return { html, error: null };
  } catch (err) {
    const message =
      err instanceof Error ? err.message.replace(/^KaTeX parse error:\s*/, "") : "Invalid LaTeX";
    return { html: "", error: message };
  }
}
