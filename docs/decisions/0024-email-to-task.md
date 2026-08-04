# ADR 0024 — Email → task: the first mail + tasks connection

Status: accepted. The first real wiring of the mail + calendar + tasks
wedge. Builds on ADR 0021 (task data model, the source-link field) and
ADR 0023 (propose-then-approve).

## Context

Tasks already stores a *source link* (`source_kind` / `source_id`) and
shows a "From an email" marker, and the propose/accept flow is live. Mail
is live. This ADR connects them: turning an email into a task, and
jumping from that task back to the email — without inventing new
architecture, only completing the wiring the task module stubbed.

The important design point is **two distinct paths**, because trust
depends on keeping them apart.

## Decision

### Two paths, kept distinct

1. **Explicit user action = direct.** A "Create task" action in mail
   (the reading pane's More menu) creates an **active** task immediately:
   `POST /tasks` with `title` = the message subject and
   `sourceKind = "email"`, `sourceId` = the message id. The user asked for
   it, so no approval step — it lands on their personal board straight
   away. This is not an AI action.

2. **AI-detected = proposed (never direct).** When the AI reads an email's
   content and *suggests* action items ("send the report by Friday"), each
   goes through `POST /tasks/propose` as a **proposed** task carrying the
   same `email` source — into the Suggestions inbox, **never** onto the
   board until the user Accepts (ADR 0023). The two paths share the data
   model (a task with a source link) and differ only in `state`: the
   explicit path writes `active`, the AI path writes `proposed`.

There is no code path where reading an email silently creates active work.

### The source-link round-trip

- **Forward** (email → task): the task stores `email` + the message id.
- **Back** (task → email): the "From an email" marker becomes a link that
  navigates to Mail with the message id (`/mail?open=<id>`); Mail resolves
  the message through the account door (`Email/get` by id), opens its
  thread, and clears the parameter. Because the resolve is tenant-scoped,
  a source id **cannot** open a message in another tenant — a foreign id
  simply resolves to nothing. Personal tasks keep the link private to
  their owner; a team task's link resolves only for viewers who can read
  the underlying message (the mail door decides, not the task).

### AI extraction endpoint

The AI path needs to turn email text into candidate tasks. That is a new
inference capability, `POST /ai/extract-tasks` (→ `alo_ai::extract_tasks`,
ADR 0011): it returns suggested titles (+ optional due dates) which the
client feeds to `/tasks/propose`. Like every AI endpoint it soft-degrades
— **it requires a configured tenant AI provider**; with none, the button
reports "AI is off" and nothing is proposed. The *destination* (proposals
→ Suggestions inbox, not the board) works regardless of the provider.

## Consequences

- **The wedge is real, not stubbed**: an email becomes a task and the task
  jumps back to the email, both directions, within the tenant.
- **Trust is preserved**: explicit user intent creates work directly; AI
  intent is always a proposal to approve. The distinction is structural
  (`state`), not a convention.
- **No cross-tenant leak**: the source link is an opaque id resolved
  through the account door; the mail door — already tenant-tested —
  governs whether the source is readable.
- **Provider dependency named**: real AI extraction needs a provider
  configured for the tenant; the explicit path and the proposal
  destination have no such dependency.

## Alternatives rejected

- **A "Create task" that lets the AI fill in an assignee/due silently** —
  rejected: mixing an explicit action with silent AI guesses blurs the two
  paths. Explicit action = exactly what the user asked (title + source);
  AI enrichment goes through the proposal path where it is reviewable.
- **Storing a copy of the email on the task** — rejected: the source is a
  reference (id), resolved live through the mail door, so it can never
  show stale content or bypass tenancy.
