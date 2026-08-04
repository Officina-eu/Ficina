# ADR 0022 — Board and list are two views of one task record

Status: accepted. Builds on ADR 0021 (task data model).

## Context

Users expect to flip between a **list** (clean rows) and a **board**
(kanban columns) and see the same work arranged two ways — instantly, and
with nothing lost. In many tools these are separate features that drift
apart. For alo they must be the same data, so a view-switch is a re-render
and never a save.

## Decision

**One set of rows (`tasks`, ADR 0021) powers both views. Neither view
stores anything of its own.**

- **Board view** = the tasks of a project, **grouped by `status`** (each
  column is a status), each column **ordered by `position`**. A card is a
  task row.
- **List view** = the **same** task rows as a flat, ordered list (default
  sort by status then position; other sorts — due date, priority — are
  client-side reorderings of the same rows, not new queries of new data).
- **Switching views is a client-side toggle.** The client already holds
  the rows; it just groups-by-status or flattens. No request, no reload,
  no write.

**Moving and reordering are single-row field updates**, never a
restructuring:

- **Drag a card to another column** → `POST /tasks/{id}/move` with the new
  `status` and a `position`; the store updates those two fields and records
  a `status_changed` activity. In the list, changing the status chip does
  the identical update.
- **Drag to reorder within a column** → the same move with a new
  `position` computed as the midpoint of its new neighbours (fractional
  index), so exactly one row changes.

The API returns tasks as plain rows; **grouping is the client's job**, so
the server never has a "board representation" to keep in sync with a "list
representation."

## Consequences

- **Instant, lossless view-switching** is structural, not best-effort.
- **A move is O(1)** — one row updated — whether it happens on the board or
  by editing the status in the list; both go through the same endpoint, so
  the two views can never disagree.
- **New views are cheap** — a calendar overlay of due tasks, a "my plate
  today" list, a per-assignee grouping are all just different groupings/
  filters of the same rows (ADR 0021's `due_at`, `assignee_user_id`).
- **The client owns arrangement**, so the board's smoothness (drag, drop,
  reorder) is a UI concern with no server round-trip beyond the one-field
  move.

## Alternatives rejected

- **A `board_columns` / `board_cards` structure separate from a `list`**
  — rejected: two sources of truth that must be reconciled, and a
  view-switch becomes a data operation. This is the exact failure mode the
  module is designed to avoid.
- **Server-side grouping (the API returns a board shape)** — rejected: it
  couples the server to one view and makes the list a second, divergent
  endpoint. Rows out, grouping in the client, keeps one truth.
