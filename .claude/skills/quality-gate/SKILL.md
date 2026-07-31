---
name: quality-gate
description: The mandatory verification gate for alo. Run before declaring ANY code change done, before every commit of production code, and whenever asked to "check", "verify", or "make sure it works". All checks must pass — zero warnings, all tests green, tenant isolation included. Never report done with a failing or skipped gate.
---

# Quality gate

"Done" is a claim about evidence. This gate is the evidence. Run every
step; a skipped step is a failed gate. Fix and re-run until fully green
— never rationalize a warning.

## Rust (`core/`, `control/`, `migrate/`)

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace --test '*tenant*'   # isolation suite, explicitly
cargo audit                                # deps with known CVEs fail the gate
```

## TypeScript (`web/`)

```
npm run lint          # eslint, zero warnings (--max-warnings 0)
npm run typecheck     # tsc --noEmit, strict
npm test
npm run build         # a frontend that doesn't build isn't done
```

## Cross-cutting

- Integration tests speak the real protocol against a running
  instance (`cargo test --test integration` / compose profile `test`).
- Grep the diff for forbidden patterns: `todo!(`, `unwrap()` outside
  `#[cfg(test)]`, `let _ =` on Results, `: any`, hardcoded
  user-facing strings, secrets/keys/tokens.
- New public surface? Confirm docs + a CHANGELOG.md line exist in the
  same diff.

## Reporting

Report the gate as a table — check, result, and for any failure the
fix applied. A green gate is the *precondition* for the done-block in
the implement skill, not a substitute for it.
