import { describe, expect, it } from "vitest";
import { detectLanguage } from "./detectLanguage";

describe("detectLanguage", () => {
  const cases: [string, string][] = [
    ['{\n  "a": 1,\n  "b": [2, 3]\n}', "json"],
    ["def greet(name):\n    print(f'hi {name}')", "python"],
    ["interface User { id: string; name: string }\nconst u: User = { id: '1', name: 'x' };", "typescript"],
    ["const x = 1;\nconsole.log(x);\nfunction f() { return x; }", "javascript"],
    ["fn main() {\n    let mut n = 0;\n    println!(\"{}\", n);\n}", "rust"],
    ["package main\nimport \"fmt\"\nfunc main() { fmt.Println(\"hi\") }", "go"],
    ["SELECT id, name FROM users WHERE age > 18 ORDER BY name;", "sql"],
    ["#!/bin/bash\nset -e\necho \"hello\"\nexport X=1", "bash"],
    [".btn { color: red; padding: 4px; }\n@media (max-width: 600px) { .btn { display: none; } }", "css"],
    ["<!DOCTYPE html>\n<html><body><div class='x'>hi</div></body></html>", "markup"],
  ];

  it.each(cases)("detects %s -> %s", (src, expected) => {
    expect(detectLanguage(src)).toBe(expected);
  });

  it("returns null for ambiguous / too-short input", () => {
    expect(detectLanguage("")).toBeNull();
    expect(detectLanguage("hello world")).toBeNull();
  });
});
