---
name: new-component
description: Scaffold a new Rust crate or TypeScript module in the Ficina monorepo with the standard structure, so one-file-one-responsibility holds from the first commit. Use when creating a new crate, service, module, or web feature area.
---

# New component scaffold

Structure decided at creation is structure you never have to untangle. Every
new component starts with responsibilities mapped to files *before* code.

## 1. Confirm it belongs

- Check `ARCHITECTURE.md`: is this within a component we build, or is it the
  job of an integrated engine? Integrated-engine functionality is reached
  through its public API from an adapter module — never reimplemented.
- Check the non-goals list. If it's there, stop and say so.

## 2. Rust crate (in `core/`, `control/`, or `migrate/`)

```
core/ficina-<name>/
├── Cargo.toml          # workspace-inherited version/edition/lints
├── src/
│   ├── lib.rs          # public API surface ONLY: re-exports + crate docs
│   ├── config.rs       # this crate's configuration struct + validation
│   ├── error.rs        # this crate's error enum (thiserror)
│   └── <one file per responsibility — named for what it does>
└── tests/
    └── <protocol/integration tests speaking real wire formats>
```

Rules baked in from commit one:

- `lib.rs` contains no logic — it defines the crate boundary.
- One error enum per crate in `error.rs`; variants carry context, messages
  are actionable.
- Workspace-level lints in the root `Cargo.toml`
  (`[workspace.lints]`: `unwrap_used = "deny"`, `todo = "deny"`,
  `unimplemented = "deny"` via clippy) so the compiler enforces CLAUDE.md.
- Binary crates get a thin `main.rs`: parse config, init `tracing`, call
  `lib`. Nothing else.
- Anything tenant-facing takes a `TenantId` parameter from the start —
  retrofitting tenancy is how cross-tenant bugs are born.

## 3. TypeScript module (in `web/`)

```
web/src/<area>/            # e.g. mail/, agenda/, admin/
├── index.ts               # public surface of the area ONLY
├── api/                   # typed JMAP/HTTP client calls, one file per resource
├── components/            # one component per file, named for what it renders
├── state/                 # stores/hooks, one concern per file
└── <area>.test.ts(x)
```

- Cross-area imports go through the other area's `index.ts` only.
- Shared UI lives in the design system (`web/src/ds/`), never copy-pasted
  between areas.
- All API types derive from one source of truth shared with the Rust side
  (generated or hand-mirrored with a round-trip test).

## 4. Wire into the whole

- Add the crate to the workspace `Cargo.toml`; add CI coverage (the pipeline
  globs the workspace, but confirm the first build runs it).
- Write the crate/module doc comment: one paragraph — what it owns, what it
  explicitly does not own, and which components it talks to.
- First commit must already pass the `quality-gate` skill, including at least
  one real test. A scaffold with no test is a stub, and stubs don't merge.
