import { describe, expect, it } from "vitest";

import {
  type DocItem,
  computeNumbering,
  referenceText,
  resolveReference,
} from "./numbering";

const eq = (id: string): DocItem => ({ id, kind: "equation" });
const table = (id: string): DocItem => ({ id, kind: "table" });
const figure = (id: string): DocItem => ({ id, kind: "figure" });
const section = (id: string, level: number): DocItem => ({ id, kind: "section", level });

describe("computeNumbering", () => {
  it("numbers each kind with its own running sequence", () => {
    const n = computeNumbering([eq("a"), table("b"), eq("c"), figure("d"), table("e")]);
    expect(n.get("a")?.display).toBe("1");
    expect(n.get("c")?.display).toBe("2");
    expect(n.get("b")?.display).toBe("1");
    expect(n.get("e")?.display).toBe("2");
    expect(n.get("d")?.display).toBe("1");
  });

  it("numbers sections hierarchically and resets deeper levels", () => {
    const n = computeNumbering([
      section("s1", 1), // 1
      section("s1a", 2), // 1.1
      section("s1b", 2), // 1.2
      section("s1b1", 3), // 1.2.1
      section("s2", 1), // 2  (resets the level-2 and level-3 counters)
      section("s2a", 2), // 2.1
    ]);
    expect(n.get("s1")?.display).toBe("1");
    expect(n.get("s1a")?.display).toBe("1.1");
    expect(n.get("s1b")?.display).toBe("1.2");
    expect(n.get("s1b1")?.display).toBe("1.2.1");
    expect(n.get("s2")?.display).toBe("2");
    expect(n.get("s2a")?.display).toBe("2.1");
  });

  it("keeps section and equation sequences independent", () => {
    const n = computeNumbering([section("s1", 1), eq("e1"), section("s2", 1), eq("e2")]);
    expect(n.get("s1")?.display).toBe("1");
    expect(n.get("e1")?.display).toBe("1");
    expect(n.get("s2")?.display).toBe("2");
    expect(n.get("e2")?.display).toBe("2");
  });
});

describe("cross-references stay correct across edits", () => {
  it("a reference resolves to the target's CURRENT number after a reorder", () => {
    const before = [eq("mass"), eq("energy"), eq("wave")];
    // "energy" is Eq. 2 to start.
    expect(resolveReference(computeNumbering(before), "energy")?.display).toBe("2");

    // Move "wave" to the front — everything renumbers, and the SAME reference
    // (by id "energy") now resolves to Eq. 3, with no edit to the reference.
    const after = [eq("wave"), eq("mass"), eq("energy")];
    expect(resolveReference(computeNumbering(after), "energy")?.display).toBe("3");
  });

  it("inserting an item ahead of the target bumps the target's number", () => {
    const base = [eq("a"), eq("b")];
    expect(resolveReference(computeNumbering(base), "b")?.display).toBe("2");
    const inserted = [eq("a"), eq("x"), eq("b")];
    expect(resolveReference(computeNumbering(inserted), "b")?.display).toBe("3");
  });

  it("resolves a dangling reference (deleted target) to null", () => {
    const n = computeNumbering([eq("a"), eq("b")]);
    expect(resolveReference(n, "gone")).toBeNull();
  });
});

describe("referenceText", () => {
  it("formats the chip label per kind", () => {
    expect(referenceText({ kind: "equation", display: "3" })).toBe("Eq. 3");
    expect(referenceText({ kind: "table", display: "1" })).toBe("Table 1");
    expect(referenceText({ kind: "figure", display: "2" })).toBe("Figure 2");
    expect(referenceText({ kind: "section", display: "2.3" })).toBe("Section 2.3");
  });

  it("uses localized labels when provided", () => {
    const fr = { section: "Section", equation: "Éq.", table: "Tableau", figure: "Figure" };
    expect(referenceText({ kind: "equation", display: "3" }, fr)).toBe("Éq. 3");
  });
});
