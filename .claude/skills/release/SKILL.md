---
name: release
description: Release workflow for alo. Use when cutting, tagging, shipping, or deploying a release, bumping versions, or writing release notes. Releases are boring by design — this checklist is what makes them boring.
---

# Cut a release

A good release is an anticlimax. Every step below exists because some
team, somewhere, learned it at 3 a.m.

## 1. Freeze and verify
- Green `quality-gate` on the exact commit being released — not a
  parent, not "main was green earlier".
- Integration suite against the composed stack (all engines, pinned
  versions from `deploy/`).
- Migration dry-run: apply to a copy of a production-shaped database;
  confirm the rollback path exists and is written down.

## 2. Version and notes
- SemVer against the *public contracts*, not the code size: breaking
  contract → major; additive → minor; fixes → patch.
- `CHANGELOG.md` is already written (implement skill guarantees it);
  edit for coherence, sort user-visible before internal.
- Note engine version bumps explicitly — operators diff these.

## 3. Ship
- Tag signed; artifacts built from the tag by CI, never from a
  laptop; SBOM attached.
- Deploy staging → smoke the golden paths (send/receive a real mail,
  JMAP session, calendar invite, chat message, file open).
- Deploy production tenant-by-tenant (own tenants first — we dogfood
  the risk before customers meet it), watching queue depth, delivery
  latency, error rates between waves.

## 4. After
- Post-release watch window with the dashboards open; the release is
  not done when deploy finishes, it is done when the graphs stay flat.
- Anything surprising → incident note, blameless, in `docs/` — even
  if customers never noticed. Surprises are free training data.
