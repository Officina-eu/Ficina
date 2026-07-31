---
name: review
description: Cold code-review workflow for alo. Use when reviewing any diff, PR, or "look at this code" request, and as the final pass of the implement skill. Adversarial stance — the reviewer's job is to find the reason NOT to merge.
---

# Review a diff cold

Read as someone who did not write it and whose name goes on the
approval. Your job is to find the reason not to merge; if after honest
effort there is none, say so plainly and approve.

## Pass 1 — the three laws (CLAUDE.md)

- Tenancy: point at the exact line where every storage access is
  scoped, and at the wrong-tenant test. No line, no approval.
- Full path: trace input → validation → logic → persistence → output
  → errors. Any `todo!()`, stray `unwrap()`, swallowed `Result`,
  or `any` fails the review.
- One responsibility: name each touched file's single reason to
  change. Two reasons → request the split, in this PR.

## Pass 2 — contracts

- Public surface changed? Additive, or versioned with a deprecation
  note. "A customer scripted yesterday's behavior — do they break?"
- Schema change? Expand → migrate → contract, reversible, never
  destructive-and-dependent in one release.
- Errors at the edge are protocol-correct (reply codes, JMAP error
  objects, HTTP statuses) and leak no internals.

## Pass 3 — the hostile reading

Think like an attacker and like Murphy: injected headers, oversized
inputs, concurrent writes, a crash between persistence steps, a
malicious tenant, a slow-loris peer. For each risk, either the diff
handles it, a test proves it, or the review asks for it.

## Pass 4 — the boring essentials

Docs/CHANGELOG in the same diff · logs clean of bodies/credentials/PII
· i18n externalized · tracing spans at boundaries · rollout answer
present (flag/default, watch-metric, off-switch).

## Verdict

End with one of exactly: **APPROVE** / **APPROVE WITH NITS** (list
them; author decides) / **REQUEST CHANGES** (each item names the file,
the problem, and the acceptance criterion). Vague discomfort is not a
verdict — convert it into a named risk or drop it.
