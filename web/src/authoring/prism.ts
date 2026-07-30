// Prism syntax highlighting (ADR 0015). Highlights code entirely in the browser
// (the source never leaves the client). The language is chosen EXPLICITLY by the
// author via the picker — never auto-detected — so a finance spec's SQL is never
// mis-read as shell. `highlight` falls back to escaped plain text for an unknown
// language, so it can never throw or inject markup.
import Prism from "prismjs";

// Core (markup/html, css, clike, javascript) ships with prismjs; the rest are
// registered by importing their component files, in dependency order.
import "prismjs/components/prism-json";
import "prismjs/components/prism-python";
import "prismjs/components/prism-rust";
import "prismjs/components/prism-sql";
import "prismjs/components/prism-bash";
import "prismjs/components/prism-yaml";
import "prismjs/components/prism-toml";
import "prismjs/components/prism-go";
import "prismjs/components/prism-java";
import "prismjs/components/prism-c";
import "prismjs/components/prism-cpp"; // depends on c
import "prismjs/components/prism-csharp";
import "prismjs/components/prism-markdown"; // depends on markup
import "prismjs/components/prism-latex";
import "prismjs/components/prism-diff";
import "prismjs/components/prism-typescript"; // depends on javascript
import "prismjs/components/prism-jsx"; // depends on markup + javascript
import "prismjs/components/prism-tsx"; // depends on jsx + typescript

export interface Language {
  /** Prism grammar id, e.g. "typescript". */
  id: string;
  /** Human label for the picker, e.g. "TypeScript". */
  label: string;
}

/** The languages offered in the picker, sorted by label. `plain` is always valid. */
export const LANGUAGES: Language[] = [
  { id: "bash", label: "Bash / Shell" },
  { id: "c", label: "C" },
  { id: "cpp", label: "C++" },
  { id: "csharp", label: "C#" },
  { id: "css", label: "CSS" },
  { id: "diff", label: "Diff" },
  { id: "go", label: "Go" },
  { id: "markup", label: "HTML / XML" },
  { id: "java", label: "Java" },
  { id: "javascript", label: "JavaScript" },
  { id: "json", label: "JSON" },
  { id: "jsx", label: "JSX" },
  { id: "latex", label: "LaTeX" },
  { id: "markdown", label: "Markdown" },
  { id: "plain", label: "Plain text" },
  { id: "python", label: "Python" },
  { id: "rust", label: "Rust" },
  { id: "sql", label: "SQL" },
  { id: "toml", label: "TOML" },
  { id: "tsx", label: "TSX" },
  { id: "typescript", label: "TypeScript" },
  { id: "yaml", label: "YAML" },
].sort((a, b) => a.label.localeCompare(b.label));

/** The picker's default language id. */
export const DEFAULT_LANGUAGE = "typescript";

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

/**
 * Highlight `code` as `langId`, returning HTML with Prism token spans. Falls
 * back to escaped plain text for "plain" or any unknown/uninstalled language.
 */
export function highlight(code: string, langId: string): string {
  const grammar = Prism.languages[langId];
  if (langId === "plain" || grammar === undefined) return escapeHtml(code);
  return Prism.highlight(code, grammar, langId);
}

/** The label for a language id, or the id itself if unknown. */
export function languageLabel(langId: string): string {
  return LANGUAGES.find((l) => l.id === langId)?.label ?? langId;
}
