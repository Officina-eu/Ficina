# ADR 0028 — Mapping team projects and shared mailboxes onto Spaces

Status: accepted. Records how the existing task "team projects" and the planned
shared mailboxes relate to the Space object (ADR 0026), and — crucially — that
the tasks retrofit is a **staged migration, not part of the Drive work**, so the
live wedge is never destabilised to ship Drive.

## Context

ADR 0026 makes Spaces the suite's membership spine. Two existing/planned
concepts overlap it:

- **Task team projects** (live). A `task_projects` row with `kind = 'team'` is
  visible to the *entire tenant* — no membership. `kind = 'personal'` is
  owner-only. The `visible_projects()` predicate encodes both.
- **Shared mailboxes** (not built). `features.md` lists them; only
  `account_delegates` (inbox delegation) exists as groundwork.

If these stay separate from Spaces, we get exactly the duplication ADR 0026
exists to prevent. But retrofitting the *live* task module onto a brand-new
Space model in the same stroke as building Drive risks the wedge for no user-
visible gain. So the decision is about **sequencing**, and it is deliberate.

## Decision

**Spaces are the target model; Drive is built on them first; existing modules
migrate onto them as separate, staged follow-ups.**

### The mapping (the target state)

- A **team project** ≙ a **Space** with the **Tasks** module enabled. Today's
  "visible to the whole tenant" becomes a Space whose membership is seeded with
  every tenant user — behaviour-preserving — after which managers can narrow it
  to real members (the capability gain).
- A **personal project** ≙ a user's **personal location**; no Space involved.
- A **shared mailbox** ≙ a **Space** with the **Mailbox** module enabled;
  delegation grants collapse into Space membership + a role. Built when the
  mailbox module is built — greenfield onto Spaces, no migration debt.

### The sequence (expand → migrate → contract, ADR-mandated for live data)

1. **Now:** ship Spaces + the Drive module on them (ADRs 0026/0027). Tasks is
   untouched and keeps `task_projects` as-is.
2. **Follow-up (own PR, own ADR update):** *expand* — introduce a nullable
   `space_id` on `task_projects`; backfill each `kind='team'` project with a
   Space seeded to all tenant users; dual-read.
3. *Migrate* — move task team-visibility reads onto Space membership behind a
   per-tenant flag, default off, then on.
4. *Contract* — once every tenant is on Spaces, retire the `kind='team'`
   tenant-wide predicate.

Each step is independently shippable and reversible; no destructive migration
rides along with a feature that depends on it.

## Rejected alternatives

- **Retrofit tasks onto Spaces in the Drive work.** Couples a live-data
  migration to a new feature; a bug in either takes down both. Violates the
  "cut scope, never depth" trade — the depth here is *keeping tasks correct*.
- **Leave team projects and Spaces permanently separate.** Two membership
  models forever; the exact duplication ADR 0026 forbids, and it would strand
  cross-module AI (ADR 0029), which needs one spine.
- **Delete team projects and force-migrate immediately.** No expand→contract
  safety; tenants run this in production, and there is no "just rerun it".

## Consequences

- Drive ships on the clean Space model now, with no risk to the live task
  wedge.
- The task module gains real per-Space membership when the migration lands —
  a user-visible improvement, delivered safely.
- The mailbox module, when built, is greenfield on Spaces and carries no
  migration debt.
- This ADR is the standing reference for the tasks retrofit; that work updates
  it rather than relitigating it.
