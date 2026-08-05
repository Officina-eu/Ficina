# ADR 0026 — The Space object: the membership spine of the suite

Status: accepted. The foundation the Drive/Docs surface (ADR 0027) attaches
to, and the object tasks, shared mailboxes, and a future feed will attach to
next. Builds on the tenant model (Law 1) and generalises the task module's
"team projects" (ADR 0021 → mapping in ADR 0028).

## Context

The workspace needs one coherent notion of "a group of people who share a
thing" — the Proceq team, the Acme project, company-wide. Microsoft splits
this into Groups, SharePoint sites, Teams, and shared mailboxes, each with its
own membership and permission model; the result is the permission maze users
hate. We want the opposite: **one group object that owns membership and
permissions, and modules that attach to it and inherit them.**

Two things already gesture at this and must be reconciled (ADR 0028):

- **Task "team projects"** — a project with `kind = 'team'` is visible to the
  *entire tenant*; there is no per-project membership. That is a Space whose
  members happen to be "everyone in the tenant".
- **Shared mailboxes** — named in `features.md` but not built; a delegation
  table (`account_delegates`) is the only groundwork.

So a Space is not a new silo bolted on — it is the general form the suite has
been approximating. Get it right once; every module reuses it.

## Decision

A **Space** is a tenant-scoped group object with explicit membership and
per-member roles. Files, docs, and later tasks/mailbox/feed are **modules** that
attach to a Space and inherit its membership — they never carry their own ACLs.

### Data model

- `spaces` — `(tenant_id → tenants ON DELETE CASCADE, id, name, created_by,
  created_at, archived)`.
- `space_members` — `(tenant_id, space_id, user_id, role, added_at)`, primary
  key `(tenant_id, space_id, user_id)`. `role ∈ {viewer, editor, manager}`.
- `space_modules` — `(tenant_id, space_id, module)`: which modules are enabled
  on the Space (e.g. `files`). A table, not a column, so enabling a module is
  additive and a new module needs no schema change to the Space itself.

Everything is keyed by `tenant_id` first; a Space, its membership, and its
modules can only ever be reached through the tenant that owns them.

### Roles (deliberately three, no matrix)

- **viewer** — read the Space's contents.
- **editor** — viewer + create / edit / upload / move / delete within the Space.
- **manager** — editor + change membership, rename/archive the Space, and
  enable/disable modules.

Roles are on the *Space*, not per file — that is the whole point. A person's
access to any file is exactly their role in the file's location.

### Personal is not a Space

A user's private "My Files" is a **personal location** owned by that user, not
a Space (mirroring personal task projects). So a thing's location is either
`personal:<user>` or `space:<id>`. This keeps "just my stuff" free of group
machinery while giving shared stuff a real group.

### The attach contract (how modules reuse this)

A module attaches to a Space by (1) keying its rows on `space_id`, (2) gating
every read on Space membership and every write on `editor`+, and (3) declaring
itself in `space_modules`. It stores **no** membership or permission state of
its own. ADR 0027 (Drive) is the first implementation and the worked example;
tasks/mailbox/feed follow the same three steps.

### Access, in one predicate

Reads: caller is a member of the Space (any role). Writes: caller is
`editor` or `manager`. Management: `manager`. A non-member — same tenant or
another tenant — gets a clean `404` (never a `403` that confirms the Space
exists; existence is itself tenant/space-private, as with tasks).

## Rejected alternatives

- **Per-file / per-resource ACLs (the SharePoint model).** Maximal
  flexibility, maximal confusion; it is the exact pain we are displacing.
  Access-follows-location (ADR 0027) is only coherent if the group owns the
  permission.
- **Reuse task "team projects" as-is for everything.** They have no membership
  (all-tenant), so they cannot express "the Acme project is these five people".
  Spaces generalise them; ADR 0028 stages the migration.
- **Roles as a free permission matrix.** Three named roles cover viewer /
  contributor / owner needs and stay explainable to a non-technical admin. A
  matrix can come later behind the same API if a real need appears.

## Consequences

- One membership model for the whole suite; a new module inherits sharing for
  free by following the attach contract.
- Spaces introduce real membership beyond today's all-tenant team projects — a
  capability gain, and the reason the tasks retrofit is a *migration* (ADR
  0028), not a rename.
- Isolation is testable by construction: the same wrong-tenant + non-member
  suite the task module runs, extended to Spaces and every attached module.
- Modules stay thin and single-responsibility (Law 3): they own their data, the
  Space owns who can touch it.
