#!/usr/bin/env python3
"""Emit a product's workspace Cargo.toml from the monorepo's (ADR 0019).

Keeps the shared [workspace.package]/[workspace.dependencies]/[lints] verbatim,
and narrows `members` to the platform crates, the product's own crates, and the
migrator — dropping the suite and every other product. Deriving it (rather than
templating) means shared dependencies never drift out of sync.

Usage: gen-manifest.py <monorepo-Cargo.toml> <product-crate-dir>
  e.g. gen-manifest.py Cargo.toml products/mail
"""
import re
import sys

# The manifest carries non-ASCII (e.g. "≥" in a comment); write UTF-8 whatever
# the platform's default stdout encoding is (Windows defaults to cp1252).
sys.stdout.reconfigure(encoding="utf-8")

cargo, crate_dir = sys.argv[1], sys.argv[2].rstrip("/")
out, in_members = [], False

for line in open(cargo, encoding="utf-8"):
    if not in_members:
        out.append(line)
        if re.match(r"\s*members\s*=\s*\[", line):
            in_members = True
        continue
    if line.strip() == "]":
        in_members = False
        out.append(line)
        continue
    m = re.match(r'\s*"([^"]+)"', line)  # a member path; comments are dropped
    if m:
        path = m.group(1)
        if path.startswith("platform/") or path.startswith(crate_dir + "/") or path == "migrate":
            out.append(line)

sys.stdout.write("".join(out))
