import { describe, expect, it } from "vitest";

import { renderMath } from "./katex";
import { highlight } from "./prism";

describe("renderMath (KaTeX)", () => {
  it("renders valid LaTeX to HTML with no error", () => {
    const r = renderMath("E = mc^2", true);
    expect(r.error).toBeNull();
    expect(r.html).toContain("katex");
  });

  it("renders a fraction (display mode)", () => {
    const r = renderMath("\\frac{a}{b}", true);
    expect(r.error).toBeNull();
    expect(r.html.length).toBeGreaterThan(0);
  });

  it("returns a structured error for invalid LaTeX instead of throwing", () => {
    const r = renderMath("\\frac{a}{", true);
    expect(r.html).toBe("");
    expect(r.error).not.toBeNull();
  });
});

describe("highlight (Prism)", () => {
  it("highlights TypeScript with token markup", () => {
    const html = highlight("const x: number = 1;", "typescript");
    expect(html).toContain("token");
  });

  it("highlights Python", () => {
    const html = highlight("def add(a, b):\n    return a + b", "python");
    expect(html).toContain("token");
  });

  it("highlights Rust", () => {
    const html = highlight('fn main() { println!("hi"); }', "rust");
    expect(html).toContain("token");
  });

  it("escapes plain text and never injects markup", () => {
    const html = highlight("<script>alert(1)</script>", "plain");
    expect(html).toBe("&lt;script&gt;alert(1)&lt;/script&gt;");
  });

  it("falls back to escaped text for an unknown language", () => {
    const html = highlight("<b>&", "no-such-lang");
    expect(html).toBe("&lt;b&gt;&amp;");
  });
});
