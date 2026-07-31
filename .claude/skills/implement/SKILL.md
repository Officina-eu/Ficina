---
name: implement
description: End-to-end feature implementation workflow for alo, run with the discipline of a senior big-tech engineering team. Use for ANY production code change — new features, endpoints, protocol commands, handlers, modules, schema changes, or extending existing ones — even small ones, and even when the user just says "add X" or "make Y work". Enforces scope-gate, design review, contract review, test-first, full-depth delivery, staged rollout thinking, and a verifiable definition of done. Triggers on "implement", "add", "build", "create", "extend", "support", "handle", or any request that ends with alo code being written.
---

# Implement like a senior team

alo is built by a small team, so we import what makes elite
engineering organizations reliable — review culture, contracts,
compatibility discipline, staged rollout, blameless verification —
and drop what makes them slow. Every role below is real; on a small
team, you play all of them in sequence. Playing a role means actually
switching stance, not skimming a checklist.

The one trade permitted everywhere: **when time is short, cut scope —
fewer commands, fewer fields — never depth.** A narrow feature that
fully works can ship; a wide one that half-works cannot.

*The trade in practice:* asked for JMAP `Email/query` with all filter
conditions, and sort logic is ballooning? Ship `from`, `to`,
`subject`, `after` at full depth — validation, tenancy, tests, docs —
and record the rest in the spec's out-of-scope list. Never ship all
conditions with `todo!()` in three of them.

## 0. Product gate — the PM's chair

Thirty seconds before anything else:

- In the current phase, and outside `Non-goals` in the product doc?
  If not, say so and stop — the default answer is no. Great teams are
  defined by what they decline to build.
- Does a module already own this? Extending an owner beats creating a
  sibling that half-overlaps it.

## 1. Design review — the tech lead's chair

Write the design where the reviewer will find it (PR description or
a `docs/` note). Short is fine; missing is not. Four blocks:

- **Surface:** inputs, outputs, who calls it.
- **Errors:** every failure condition and exactly what the caller
  sees for each.
- **Tenancy:** how every read and write is scoped to a tenant. If the
  answer is "it isn't", the design is wrong, not unfinished.
- **Out of scope:** what you are deliberately not building — cuts are
  decisions, and decisions are written down.

State the *alternative you rejected* in one sentence. If there was no
alternative, you haven't thought about it yet — that sentence is the
entire value of design review.

For protocol work, cite exact RFC sections (use the `protocol`
skill). When an RFC and a real client disagree: strict in what we
send, tolerant in what we accept, and the deviation is recorded in
`docs/interop.md` — the next engineer inherits the knowledge, not the
debugging session.

## 2. Contract review — the API board's chair

Anything crossing a public surface (JMAP method, HTTP route, config
key, CLI flag, event schema) is a contract, and contracts outlive
code:

- **Naming** follows the platform's existing conventions exactly —
  consistency is a feature users feel and never report.
- **Compatibility is sacred.** Additive changes only on live
  surfaces; a break requires versioning and a deprecation note, never
  a silent change. Ask: "if a customer scripted against yesterday's
  behavior, does today break them?"
- **Schema changes** are expand → migrate → contract across separate
  releases, never a destructive migration in the same change that
  depends on it. Tenants run this in production; there is no "just
  rerun it".
- **User-facing strings** are externalized for i18n from day one —
  alo is European; hardcoded English is a bug, not a default.

## 3. Tests first — the QA engineer's chair

- Unit tests for logic; an integration test speaking the real
  protocol (SMTP/JMAP/IMAP/DAV/HTTP) against a running instance for
  behavior.
- Error-path tests in the same pass as happy-path: malformed input,
  oversized input, unauthenticated, mid-operation failure.
- **The wrong-tenant test is mandatory for anything touching
  storage** — tenant A addressing tenant B's data gets a clean
  denial, not data and not a 500. A tenant leak is the one bug
  category we cannot apologize for.
- Run the tests; confirm they fail *for the right reason* — a test
  failing on a typo proves nothing.

## 4. Implement full depth — the engineer's chair

Work the entire chain in one arc:

```
input → validation → logic → persistence → output → error paths
```

- Typed errors (`thiserror`), propagated with `?`, mapped to
  protocol-correct responses at the edge (SMTP reply codes, JMAP
  error objects, HTTP statuses). The wire never sees a raw internal
  error.
- No `todo!()`, no `unwrap()` outside tests, no `let _ =` on a
  `Result`, no `any` in TypeScript. The pull to stub is the signal
  the scope is too big — return to the trade: cut a field, finish the
  rest fully.
- **Instrument as you build,** not after: structured `tracing` spans
  at the boundaries, a counter or histogram for anything an operator
  will one day graph. Elite teams ship the dashboard with the
  feature. Never log message bodies, credentials, or personal data —
  our logs are held to the promise we sell.
- **New behavior that could disrupt existing tenants ships behind a
  flag or per-tenant setting,** default off. Rollout is then a
  config change, rollback is the same config change, and no deploy is
  an event.

## 5. Verify — the SRE's chair

Run the `quality-gate` skill; all of it must pass — not "compiles",
but zero warnings, formatted, all tests green, tenant-isolation tests
included.

Then one honest manual pass: drive the real path with a real client
or `curl`/`swaks`/protocol tool and **read the actual bytes on the
wire**. Automated green catches regressions; reading the wire catches
the bug nobody thought to test — wrong reply code, missing header,
off-by-one in a size limit. Paste the wire exchange into the PR:
review becomes evidence instead of trust.

Before declaring safe, answer the SRE's two questions in the PR:
*how will we notice if this misbehaves in production* (which metric,
which alert), and *how do we turn it off* (flag, config, revert).

## 6. Ship — the reviewer's chair

Read the full diff cold, as a reviewer who didn't write it, against
the three laws in CLAUDE.md. Any file with more than one reason to
change gets split before merge, not "later".

- Rustdoc/TSDoc on every public item; `docs/` updated for
  operator- or user-visible changes; `docs/interop.md` for client
  quirks; `ARCHITECTURE.md` in the same PR if the design contract
  moved. Documentation in a later PR is documentation that never
  lands.
- A user-readable line in `CHANGELOG.md` — release notes are written
  when the knowledge is fresh, not reconstructed at release time.

Close with the done-block, every line answered:

```
Implemented:   <what exists now>
Out of scope:  <what was cut, and where it's recorded>
Verified:      <tests + the manual wire pass>
Tenancy:       <how isolation is enforced and tested>
Rollout:       <flag/default, the watch-metric, the off-switch>
Docs:          <what was updated, incl. changelog>
```

A line you can't fill is pointing at the step you skipped — go back
to that step. That honesty, applied every time without exception, is
the entire difference between a professional team and a fast one.
