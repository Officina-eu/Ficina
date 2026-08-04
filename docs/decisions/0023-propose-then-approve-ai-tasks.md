# ADR 0023 — Propose-then-approve for AI-created tasks

Status: accepted. alo's signature pattern for AI that creates things.

## Context

A defining alo feature is turning a meeting into action items and an email
into a task. But AI is fallible, and a task system that silently fills up
with machine-guessed items is worse than useless — it erodes trust and
buries the real work. Across every module, **alo's rule is that AI
proposes and the human approves.** This ADR fixes that rule for tasks so
the meeting→action-items and email→task flows are built on it from day one,
even before those source modules are fully wired.

## Decision

**AI never creates an active task. It creates a *proposal*, and the user
accepts or rejects it.**

Mechanically this reuses ADR 0021's `state` column — no separate table, no
divergent model:

1. **Propose.** `POST /tasks/propose` takes one or more suggested tasks
   (title, and optionally a suggested `assignee`, `due_at`, `priority`, and
   the `source` — e.g. the meeting/event or email) and inserts each as a
   task with **`state = 'proposed'`**. A proposed task is a normal task
   row, carrying its source link and the AI's suggestions.
2. **Never surfaced as work.** Normal list/board/"my plate" queries return
   only `state = 'active'` rows, so a proposal never appears as if it were
   real work. Proposals live in their own review surface
   (`GET /tasks/proposals`).
3. **Approve.** `POST /tasks/{id}/accept` flips the row to `state =
   'active'` (optionally with the user's edits to assignee/due/priority) and
   records an `accepted` activity — the moment a human took ownership.
4. **Reject.** `POST /tasks/{id}/reject` deletes the proposal. Nothing
   half-created is left behind.

The **review UI** shows each proposal with its source ("from the 10:00
sync"), the suggested assignee and due date, and Accept / Edit / Reject —
so approving the real ones and dropping the noise is a few clicks, and the
AI's guess is always visible and always overridable.

This pattern applies to **any** AI task creation in alo — meeting action
items, email→task suggestions, and whatever comes next. There is no code
path where the AI writes an `active` task directly.

## Consequences

- **Trust.** The task list only ever contains work a human chose to keep.
- **The hooks exist now.** `POST /tasks/propose` + the proposals surface are
  built and testable immediately; the meeting-transcript and email sources
  plug into `propose` as those modules land — the approval half is done.
- **One model.** Proposals are tasks in a different `state`, so accepting
  is a field flip and a proposal already has its source link and can be
  edited before acceptance — no copying between a "suggestions" table and a
  "tasks" table.
- **Auditable.** Accept/reject are recorded in task activity, so it is
  always clear which items were machine-suggested and who approved them.

## Alternatives rejected

- **AI writes tasks directly, user deletes the wrong ones** — rejected:
  opt-out is the wrong default for machine-generated work; it buries real
  tasks under guesses and burns trust. Opt-in (approve) is the rule.
- **A separate `task_suggestions` table** — rejected: it duplicates the
  task shape, needs its own source/assignee/due columns, and makes
  acceptance a copy instead of a one-field state change.
