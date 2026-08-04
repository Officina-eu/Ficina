# ADR 0021 — Task data model: one record, projects, and a source link

Status: accepted (product owner directed the Tasks module — the third leg
of the mail + calendar + tasks wedge).

## Context

alo is adding task management as a first-class module, not a bolted-on
to-do list. It must be personal AND team, tenant-scoped like everything
else, on the same Rust + Postgres backend and the same OIDC identity, and
it must connect to mail and calendar (a task can come from an email or a
meeting, and a task with a due date can surface on the calendar).

The single most important decision is the shape of the data, because two
downstream decisions depend on it: that **board and list are two views of
one record** (ADR 0022) and that **AI-created tasks are proposed, not
silently created** (ADR 0023).

## Decision

A **task is one row.** Everything a view needs is a column on that row, so
switching views or moving a card is a field update, never a data
migration.

**`tasks`** (the core record), tenant-scoped by construction:

- `tenant_id`, `id`
- `project_id` — the board/project it lives on (see projects below)
- `title`, `description`
- `status` — the board column (`todo` / `in_progress` / `done` by
  default; a free-text string so custom columns are possible later). The
  board groups by this; the list shows it as a chip.
- `position` — a fractional order (`double precision`) **within its
  status**; drag-reorder inserts between two cards by averaging, so a move
  touches one row.
- `assignee_user_id` (nullable)
- `due_at` (nullable, UTC) — what the calendar reads to surface due tasks
- `priority` — `none` / `low` / `medium` / `high`
- `state` — `active` or `proposed`. **A proposed task never appears in a
  normal list/board** (ADR 0023); accepting flips it to `active`.
- `source_kind` / `source_id` (nullable) — the **source link**: the email
  message or calendar event this task was created from. Stored now; the
  jump-to-source UI lights up as mail/calendar wire in.
- `created_by`, `created_at`, `updated_at`, `completed_at` (nullable)

**Projects** group tasks and are how *personal vs team* is expressed with
**one** model, not two:

- **`task_projects`**: `tenant_id`, `id`, `name`, `kind`
  (`personal` / `team`), `owner_user_id`, `color`, `archived`.
- A **personal** project is auto-created per user (deterministic id
  `proj_personal_<user>`), visible only to its owner — the private
  to-do list.
- A **team** project is created explicitly and shared. (v1 scopes team
  projects as visible to the whole tenant; per-project membership is a
  follow-up, mirroring how calendar sharing grew — flagged, not faked.)

**Child tables** (each tenant-scoped, each referencing a task):

- `task_subtasks` — a lightweight checklist (`title`, `done`, `position`).
- `task_comments` — `author_user_id`, `body`, `created_at`.
- `task_activity` — the history: `actor_user_id`, `kind`
  (`created` / `status_changed` / `assigned` / `due_changed` /
  `commented` / …), `detail` (JSONB), `created_at`. Written by the store
  on every change, so the panel's activity feed is a read, not a
  reconstruction.
- `task_attachments` — `blob_id`, `filename`, `size` (reuses the existing
  tenant blob store; upload wiring is a follow-up).

## Consequences

- **List and board are lossless views of the same rows** (ADR 0022) —
  guaranteed by the schema, not by careful client code.
- **Personal and team are one system** — same `tasks`, same API, differing
  only by which project (and thus scope) a task belongs to.
- **The connections have a home now**: `source_kind`/`source_id` is the
  email/meeting link; `due_at` is what the calendar overlays; `state`
  carries the propose-then-approve flow. The hooks exist in the data even
  where the other module's wiring is still to come.
- **Tenancy** is inherited from the account door: every task query carries
  `tenant_id`, and visibility is by project ownership/scope — a personal
  project resolves only for its owner.

## Alternatives rejected

- **Separate board and list storage** — rejected: it makes the two views
  diverge and a view-switch a data operation. The whole point is one
  record (ADR 0022).
- **Subtasks as full nested tasks (parent_id)** — deferred: a checklist
  covers the v1 need without recursive queries; real sub-tasks-as-tasks
  can arrive later without breaking the checklist.
- **Integer positions with renumbering** — rejected in favour of
  fractional positions, so a reorder writes one row instead of resequencing
  a column.
