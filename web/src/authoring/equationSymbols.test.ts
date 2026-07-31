import { describe, expect, it } from "vitest";
import katex from "katex";

import { EQ_CATEGORIES, haystack, insertText } from "./equationSymbols";

// Every symbol in the picker must produce LaTeX that KaTeX can render — an entry
// that throws would give the user a broken "can't render" preview. Templates are
// rendered next to a base (`x`) so subscript/superscript/root placeholders have
// something to attach to, exactly as they do when inserted mid-expression.
describe("equation symbol catalogue", () => {
  const all = EQ_CATEGORIES.flatMap((c) => c.symbols);

  it("has a healthy number of symbols across categories", () => {
    expect(EQ_CATEGORIES.length).toBeGreaterThanOrEqual(8);
    expect(all.length).toBeGreaterThan(150);
  });

  it("renders every inserted symbol without error", () => {
    const broken: string[] = [];
    for (const s of all) {
      try {
        katex.renderToString(`x${insertText(s)}`, { throwOnError: true, displayMode: true });
      } catch (e) {
        broken.push(`${s.name} (${s.latex}): ${(e as Error).message.split("\n")[0]}`);
      }
    }
    expect(broken).toEqual([]);
  });

  it("is searchable — the haystack covers name, command and keywords", () => {
    const sum = all.find((s) => s.name === "sum");
    expect(sum).toBeDefined();
    if (sum !== undefined) {
      expect(haystack(sum)).toContain("sum");
      expect(haystack(sum)).toContain("sigma");
    }
  });
});
