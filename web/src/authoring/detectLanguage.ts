// Best-effort language detection for pasted code. A lightweight signature
// scorer over the languages the picker offers — never perfect, but a good guess
// the user can still override. Returns a language id, or `null` when nothing
// scores confidently (the caller then leaves the current choice alone).
import { LANGUAGES } from "./prism";

interface Rule {
  re: RegExp;
  w: number;
}

const SIGNATURES: Record<string, Rule[]> = {
  python: [
    { re: /^\s*def\s+\w+\s*\(.*\)\s*:/m, w: 3 },
    { re: /^\s*(from|import)\s+\w+/m, w: 2 },
    { re: /\bself\b/, w: 1 },
    { re: /^\s*(elif|except|print)\b|:\s*$/m, w: 1 },
  ],
  typescript: [
    { re: /\binterface\s+\w+/, w: 3 },
    { re: /:\s*(string|number|boolean|void|any|unknown)\b/, w: 2 },
    { re: /\btype\s+\w+\s*=/, w: 2 },
    { re: /\b(enum|implements|declare|readonly)\b/, w: 2 },
    { re: /\bas\s+const\b/, w: 2 },
  ],
  javascript: [
    { re: /\b(const|let|var)\s+\w+\s*=/, w: 2 },
    { re: /\bfunction\b|=>/, w: 1 },
    { re: /\b(console\.log|require|module\.exports|document|window)\b/, w: 2 },
  ],
  rust: [
    { re: /\bfn\s+\w+/, w: 2 },
    { re: /\blet\s+mut\b|\bimpl\b|\bpub\s+fn\b/, w: 3 },
    { re: /\bprintln!|\buse\s+\w+::/, w: 2 },
    { re: /->\s*\w+|::/, w: 1 },
  ],
  go: [
    { re: /^\s*package\s+\w+/m, w: 3 },
    { re: /\bfunc\s+\w*\s*\(/, w: 2 },
    { re: /:=/, w: 2 },
    { re: /\bfmt\.\w+|\bimport\s*\(/, w: 2 },
  ],
  cpp: [
    { re: /#include\s*<\w+>/, w: 2 },
    { re: /\bstd::|::\w+|\bcout\b|\bnamespace\b/, w: 3 },
    { re: /\btemplate\s*</, w: 2 },
  ],
  c: [
    { re: /#include\s*<\w+\.h>/, w: 2 },
    { re: /\bint\s+main\s*\(/, w: 2 },
    { re: /\bprintf\s*\(/, w: 2 },
  ],
  csharp: [
    { re: /\busing\s+System/, w: 3 },
    { re: /\bnamespace\s+\w+|\bConsole\.Write/, w: 2 },
    { re: /\bpublic\s+(class|static\s+void\s+Main)/, w: 2 },
  ],
  java: [
    { re: /\bpublic\s+(class|static\s+void\s+main)/, w: 3 },
    { re: /\bSystem\.out\.print/, w: 3 },
    { re: /\bimport\s+java\./, w: 3 },
  ],
  sql: [
    { re: /\bSELECT\b[\s\S]*\bFROM\b/i, w: 3 },
    { re: /\b(INSERT\s+INTO|UPDATE|DELETE\s+FROM|CREATE\s+TABLE|WHERE|JOIN)\b/i, w: 2 },
  ],
  bash: [
    { re: /^#!.*\b(bash|sh|zsh)\b/m, w: 3 },
    { re: /^\s*(echo|export|cd|sudo|apt|npm|git|curl|chmod)\s/m, w: 2 },
    { re: /\$\{?\w+\}?|\|\s*grep/, w: 1 },
  ],
  css: [
    { re: /[.#]?[\w-]+\s*\{[^}]*:[^}]*;/, w: 3 },
    { re: /@(media|import|keyframes|font-face)\b/, w: 2 },
    { re: /\b(color|margin|padding|display|background|font-size)\s*:/, w: 1 },
  ],
  json: [
    { re: /^\s*[[{][\s\S]*[\]}]\s*$/, w: 1 },
  ],
  markup: [
    { re: /<\/?[a-z][\w-]*(\s[^>]*)?>/i, w: 2 },
    { re: /<!DOCTYPE|<html|<div|<span|<\?xml/i, w: 3 },
  ],
  yaml: [
    { re: /^\s*[\w-]+:\s*(.+)?$/m, w: 1 },
    { re: /^---\s*$/m, w: 2 },
    { re: /^\s*-\s+\w+/m, w: 1 },
  ],
  toml: [
    { re: /^\s*\[[\w.]+\]\s*$/m, w: 3 },
    { re: /^\s*[\w-]+\s*=\s*.+$/m, w: 1 },
  ],
  markdown: [
    { re: /^#{1,6}\s+\S/m, w: 3 },
    { re: /^\s*[-*+]\s+\S|^\s*\d+\.\s+\S/m, w: 1 },
    { re: /```|\[[^\]]+\]\([^)]+\)/, w: 2 },
  ],
  latex: [
    { re: /\\(begin|end|documentclass|section|usepackage)\b/, w: 3 },
    { re: /\\[a-zA-Z]+\{/, w: 1 },
  ],
  diff: [
    { re: /^@@\s.*@@/m, w: 3 },
    { re: /^[+-].*$/m, w: 1 },
  ],
};

const KNOWN = new Set(LANGUAGES.map((l) => l.id));

/** Detect the language of `text`, or `null` if no confident match. */
export function detectLanguage(text: string): string | null {
  const src = text.slice(0, 4000); // a sample is plenty
  if (src.trim().length < 3) return null;

  // JSON is a strong, verifiable signal — prefer it when it actually parses.
  const trimmed = src.trim();
  if ((trimmed.startsWith("{") || trimmed.startsWith("[")) && isJson(trimmed)) {
    return "json";
  }

  let best: string | null = null;
  let bestScore = 0;
  for (const [lang, rules] of Object.entries(SIGNATURES)) {
    if (!KNOWN.has(lang)) continue;
    let score = 0;
    for (const { re, w } of rules) if (re.test(src)) score += w;
    // JSX/TSX: JS/TS plus React-style tags.
    if (score > bestScore) {
      bestScore = score;
      best = lang;
    }
  }

  // Refine JS/TS to their JSX/TSX variants when the sample has React markup.
  if ((best === "javascript" || best === "typescript") && /<[A-Z][\w.]*[\s/>]|<>\s*</.test(src)) {
    best = best === "typescript" ? "tsx" : "jsx";
  }

  return bestScore >= 3 ? best : null;
}

function isJson(s: string): boolean {
  try {
    JSON.parse(s);
    return true;
  } catch {
    return false;
  }
}
