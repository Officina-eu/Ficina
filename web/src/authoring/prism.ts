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
  /** 1–2 character badge shown in the picker (e.g. "Py", "{}"). */
  badge: string;
  /** Badge background colour. */
  badgeBg: string;
}

/** The languages offered in the picker, sorted by label. `plain` is always valid. */
export const LANGUAGES: Language[] = [
  { id: "bash", label: "Bash / Shell", badge: "$_", badgeBg: "#4b5563" },
  { id: "c", label: "C", badge: "C", badgeBg: "#5c6bc0" },
  { id: "cpp", label: "C++", badge: "C+", badgeBg: "#5c6bc0" },
  { id: "csharp", label: "C#", badge: "C#", badgeBg: "#6a4c93" },
  { id: "css", label: "CSS", badge: "#", badgeBg: "#2965f1" },
  { id: "diff", label: "Diff", badge: "±", badgeBg: "#4b5563" },
  { id: "go", label: "Go", badge: "Go", badgeBg: "#00acd7" },
  { id: "markup", label: "HTML / XML", badge: "<>", badgeBg: "#e34c26" },
  { id: "java", label: "Java", badge: "Jv", badgeBg: "#d97706" },
  { id: "javascript", label: "JavaScript", badge: "JS", badgeBg: "#d4a72c" },
  { id: "json", label: "JSON", badge: "{}", badgeBg: "#6b7280" },
  { id: "jsx", label: "JSX", badge: "JX", badgeBg: "#61dafb" },
  { id: "latex", label: "LaTeX", badge: "Tex", badgeBg: "#008080" },
  { id: "markdown", label: "Markdown", badge: "M↓", badgeBg: "#4b5563" },
  { id: "plain", label: "Plain text", badge: "T", badgeBg: "#6b7280" },
  { id: "python", label: "Python", badge: "Py", badgeBg: "#3572A5" },
  { id: "rust", label: "Rust", badge: "Rs", badgeBg: "#b7410e" },
  { id: "sql", label: "SQL", badge: "DB", badgeBg: "#336791" },
  { id: "toml", label: "TOML", badge: "Tm", badgeBg: "#9c4221" },
  { id: "tsx", label: "TSX", badge: "TX", badgeBg: "#3178c6" },
  { id: "typescript", label: "TypeScript", badge: "TS", badgeBg: "#3178c6" },
  { id: "yaml", label: "YAML", badge: "Y", badgeBg: "#cb171e" },
].sort((a, b) => a.label.localeCompare(b.label));

/** The badge (text + colour) for a language id, for the picker and header pill. */
export function languageBadge(langId: string): { badge: string; badgeBg: string } {
  const l = LANGUAGES.find((x) => x.id === langId);
  return l !== undefined
    ? { badge: l.badge, badgeBg: l.badgeBg }
    : { badge: "T", badgeBg: "#6b7280" };
}

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
