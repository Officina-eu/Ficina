# Design note — alo Projects & Timesheets (client work, hours, and the invoice they become)

Status: **as built** (B3.01 design, reconciled against the code at B3.11
on 2026-08-08) · ADR 0035 · Business track wave B3

alo Projects is the third Work OS module, and the first one that does not
start with a new noun. The workspace already has projects: `task_projects`
is the board a team's tasks live on (ADR 0021, ADR 0022), shipped and in
daily use. What wave B3 adds is a **second lens on those same rows** — the
customer they are worked for, the budget they are worked against, the rate
they are worked at — plus the thing no lens can supply: the **hours**, and
the arc that turns them into money.

That arc is the module's reason to exist, and it is one sentence long: a
timer runs, the minutes land on a project, the week is approved, the
approved hours become the lines of a **draft** invoice in alo Billing, and
the invoice is issued by a human. Every decision below is answerable by
asking which step of that sentence it protects.

This note records the surface, the data model, the arithmetic (this is the
first module where the *conversion* of a stored integer is a design
question worth a section), the error map, the tenancy rules — including the
one this module adds to alo's vocabulary, that **a person's hours are
personal data even inside their own tenant** — and every central decision
with the alternative it rejects. Paragraphs marked **as built at B3.11**
record where the code and this note disagreed, and the code won.

> **Wave gate, flagged for a human.** `ROADMAP.md` gates wave B2 on "B1 live
> with ≥1 real tenant", and B1, B2 and BI-1 are all code-complete and
> undeployed. This note is design work, which is exactly what belongs ahead
> of an unmet gate; **B3.02 is the first item that writes a migration**, and
> a human should confirm or move the gate before it ships. Recorded in
> `docs/autonomy/STATE.md` rather than decided here.

## Surface

- **Inputs:** authenticated workspace users driving `/projects/*` on
  `alo-jmap` — the client facts of a project, milestones, templates, the
  running timer, time entries, the weekly submit, an admin's approve/reject,
  the unbilled view, the handoff to billing, and the profitability report.
  The Projects agent (ADR 0034, item B3.10) is a second caller of the same
  store functions, never of a parallel code path.
- **Outputs:** JSON resources; CSV for the profitability report; **draft**
  invoices in alo Billing (never issued, never sent); **proposed** time
  entries when the agent drafts a timesheet, which are not hours until a
  human accepts them.
- **Who calls it:** `web/src/projects` (the module UI, B3.07) and one shell
  widget (the running timer, visible from every module) call `alo-jmap`; the
  `alo-ai` projects module produces propose-then-approve envelopes that
  `alo-jmap` executes. Nothing external calls Projects.

`/projects` is a **new top-level route prefix**: the production Caddyfile
needs it added at the next deploy, the same standing human action
`/billing`, `/crm` and `/insights` carry, and it must join
`API_PATHS` in `web/vite.config.ts` at B3.04 or every call 404s into the
dev SPA (the lesson S1.11 and BI1.04 both paid for). Noted in STATE.md at
B3.04, not touched by the loop.

The prefix doubles as the SPA path, exactly as `/billing`, `/crm`,
`/insights` and `/sites` do: the dev proxy bypasses itself for HTML
navigations, so one word serves the API and the router without a second
name to keep in sync.

### Routes

All under the authenticated `alo-jmap` router, following the convention
`/billing/*` established and `/crm/*` confirmed: the `authenticate`
extractor, typed `Problem` errors, the store-error map in
[`billing::map_store_err`], registration in `server.rs`.

*As built at B3.11: the table below is the router.* Four spellings moved
between the design and the code, each for a reason recorded beside it — the
client facts and the milestones because `audit_action::event_for` derives a
record's history mechanically from the matched route template and needs the
**collection in the second segment** (`/projects/{id}/client` derives to no
audit action at all, and `tests/audit_routes.rs` fails the build for it); the
proposal seam because a drafted hour arrives through the agent's own execute
route and never needed a verb of its own; and reaching a milestone because a
delivered date is not an edit.

| Route | Purpose |
|---|---|
| `GET /projects` | the engagement list: every project this user can see, with its client facts, hours to date and budget consumption (B3.02, B3.08) |
| `GET /projects/{id}` | one engagement — the same row a task board shows, seen as client work |
| `PUT /projects/clients/{id}` | set or replace the client facts (customer, currency, rate, budget, start date). Idempotent, so a UI that saves a whole form has one call, not a create/update pair. `{id}` is the **project's** id; `clients` is the collection the audit derivation reads |
| `DELETE /projects/clients/{id}` | make it an internal project again. The hours stay; what is deleted is the *claim that they are billable to somebody* |
| `GET/POST /projects/milestones` · `GET/PATCH/DELETE /projects/milestones/{mid}` | the plan of one project (`?projectId=` on the list, `projectId` in the body) — the milestones and the task→milestone links in one read, so a timeline never draws a bar before it knows what is under it (B3.09a) |
| `POST /projects/milestones/{mid}/done` | reach a milestone, or take the mark off. Its own route, so the trail says `projects.milestone.done` rather than filing an accepted deliverable as an edit |
| `PUT/DELETE /projects/tasks/{task_id}/milestone` | put one task under a milestone, or take it out (B3.09a). One milestone per task |
| `GET/POST /projects/templates` · `DELETE /projects/templates/{id}` | mark a project as a reusable engagement template, list them, un-mark (B3.09b). `{id}` is the **project's own** id — a template *is* a project, so there is no second record to go stale |
| `POST /projects/templates/{id}/instantiate` | create a new project from a template, with a start date (B3.09b) |
| `GET /projects/timer` | the caller's running timer, or `null` |
| `POST /projects/timer/start` · `POST /projects/timer/stop` | start it on a project (optionally a task); stop it, which is what writes the entry (B3.04) |
| `GET /projects/time?from&to[&project_id]` | the caller's own entries in a date range (B3.04) |
| `POST /projects/time` | a manual entry: a date, minutes, a project (B3.04) |
| `GET/PATCH/DELETE /projects/time/{eid}` | read, correct, remove one of the caller's own entries — while its week is unlocked (B3.03, B3.05) |
| `GET /projects/time/proposals` · `POST /projects/time/{eid}/accept` · `/reject` | the agent's drafted entries and the two answers a human gives them (B3.10a). **There is no `POST /projects/time/propose`:** the only thing that drafts an hour is a tool call through `/ai/agent/execute`, which reaches the same store function, so a second door would have been an unowned write path |
| `GET /projects/weeks?from&to` | the caller's own weeks and their status (B3.05) |
| `POST /projects/weeks/{monday}/submit` · `/withdraw` | submit a week for approval; take it back while nobody has decided (B3.05) |
| `GET /projects/approvals` | **admin:** the submitted weeks of every user, oldest first (B3.05) |
| `POST /projects/approvals/{wid}/approve` · `/reject` · `/reopen` | **admin:** the decision, and the way back from one (B3.05) |
| `GET /projects/unbilled?customer_id=&to=` | approved, billable, unbilled entries for one customer, already grouped the way an invoice would group them, with the money each group would carry (B3.06) |
| `POST /projects/invoices` | raise a **draft** invoice in billing from a selection of those entries (B3.06) |
| `GET /projects/reports/profitability?from&to[&project_id]` · `.csv` | hours × rates vs budget, per project, per currency (B3.08) |

Eleven path segments are reserved words under `/projects` — `clients`,
`time`, `timer`, `weeks`, `approvals`, `templates`, `milestones`, `tasks`,
`unbilled`, `invoices`, `reports` (`clients` joined them at B3.07 with the
route rename above; the design listed ten and miscounted them as seven).
Ids are base64url'd 16-byte random
tokens (`id.rs`), so a project can never *be* one of them, and matchit
prefers a static segment to a capture; this is the shape `/tasks/labels`
beside `/tasks/{id}` and `/sites/config` beside `/sites/{id}` already have.

A week is addressed by its **Monday** (`/projects/weeks/2026-08-03/submit`)
rather than by a row id, because a week the user has never submitted has no
row yet and asking them to create one first would be a round trip that
exists only to satisfy REST. The admin routes address the row, because by
then it certainly exists.

### Web surface

`web/src/projects`, the module pattern Billing, CRM and Insights share: one
`ProjectsModule.tsx` owning the tab layout, one `api.ts` owning the fetches,
one view per screen, `.module.css` for layout, `ds` tokens for everything
else, and every string through `i18n/en.ts` under a `projects*` prefix
(fr/nl at the wave review, B3.11).

- **Projects** — the engagement list: customer, currency, hours logged, a
  budget bar, and the last time anybody worked on it.
- **My week** — the timesheet grid: rows are projects, columns are the seven
  days, cells are minutes; the row total, the day total and the week total
  are read from the server. Submit sits at the bottom with the week's status
  beside it.
- **Approvals** — admin only, and hidden entirely when the caller is not an
  admin rather than shown disabled: a list of submitted weeks with the
  person, the week, the total, and approve/reject.
- **Unbilled** — pick a customer, see the groups an invoice would carry,
  select, raise the draft; the draft opens in Billing, which owns it from
  that moment.
- **Reports** — profitability per project with the CSV export beside it.

Plus **one component outside the module**: the running-timer widget in
`web/src/shell`, because a timer you cannot see from your inbox is a timer
you forget to stop. It shows the elapsed minutes, the project, and a stop
button; it polls nothing when no timer runs.

**As built at B3.11.** Five tabs shipped, not five screens as listed:
**Projects**, **My week**, **Plan** (B3.09a's timeline, which the design had
as a rendering rather than a tab), **Reports** and **Approvals**. Three
differences worth a reader's time:

- **The Unbilled screen is not built.** `GET /projects/unbilled` and
  `POST /projects/invoices` exist, are wire-verified end to end and are
  reachable by any client; what has no browser screen is the *selecting and
  raising*. The report's **To invoice** column is where a person sees the
  money waiting, and the draft is raised through the API. This is the wave's
  one named feature cut — it is in the table at the end of this note, and in
  `docs/features.md`.
- **The timer widget lives in `web/src/projects/TimerWidget.tsx`**, declared
  by `product/workplace.tsx` as a rail widget, not in `web/src/shell`. Written
  the design's way, the shell would import `../projects` — and the shell is
  shared with alomails, which has no Projects at all. The rail renders
  `surface.railWidgets` generically and knows nothing about clocks; a
  `window`-event bus (`timerBus.ts`, no payload) tells the two trees to re-read
  from the server, which is the only thing that knows whether a stop landed.
- **The week grid carries a list of the week's entries beneath it.** The grid
  alone cannot address the second of two sittings on one project on one day,
  and merging them would erase the notes. Proposed entries appear in the grid
  marked and counted separately, but are never edited through it: a suggestion
  is accepted or rejected (ADR 0023's three verbs), not corrected.

The rail entry is a workspace-surface module (`web/src/product/workplace.tsx`),
beside Billing, CRM, Insights and Sites and for the same reason: the
business suite is what aloworkplace.com sells and it has no place in the
standalone mail app.

**A naming honesty note.** The rail will read "Projects" while the Tasks
module also calls its boards projects — they are the same rows, which is the
whole point, but a stranger reading two tabs deserves to know that. The
copy inside this module says *client project* wherever the distinction
carries weight (the list header, the client-facts form, the report), and
the Tasks module's own strings are left alone: churning another module's
copy to disambiguate mine is how a small team spends a week on nouns.

## Data model

Five new tables, all `REFERENCES tenants(id) ON DELETE CASCADE` (the
migration 0048 pattern — nothing survives a tenant delete), primary keys
`(tenant_id, id)`, ids opaque. The last migration is
`0121_insight_seeds.sql`, so B3 starts at **0122**.

```
project_clients      one row per project that is client work (grain: project)
  tenant_id, project_id  PK
  customer_id            → billing_customers (this tenant's)
  currency               ISO 4217, snapshotted from the customer
  rate_cents             default hourly rate, integer cents, nullable
  budget_minutes         nullable
  budget_cents           nullable
  starts_on              DATE, nullable
  created_at, updated_at

time_entries         one row per completed piece of work (grain: entry)
  tenant_id, id       PK
  user_id             who worked
  project_id          → task_projects
  task_id             nullable → tasks
  work_date           DATE — the day the person says they worked
  started_at          TIMESTAMPTZ nullable — set by the timer, else null
  minutes             INT, 1…1440
  billable            BOOLEAN
  rate_cents          nullable, snapshotted at write
  currency            nullable, snapshotted with the rate
  note                TEXT
  state               'active' | 'proposed'   (ADR 0023)
  source_kind/_id     nullable — 'event' for a calendar-drafted entry
  invoice_id          nullable → billing_invoices; set when billed
  billed_at           nullable
  created_by, created_at, updated_at

time_timers          the running timer (grain: user — one row, or none)
  tenant_id, user_id  PK
  project_id, task_id, started_at, billable, note

time_weeks           one person's week, once it has a status (grain: user × week)
  tenant_id, id       PK,  UNIQUE (tenant_id, user_id, week_start)
  user_id, week_start DATE (always a Monday)
  status              'open' | 'submitted' | 'approved' | 'rejected'
  submitted_at, decided_by, decided_at, decision_note

project_milestones   (grain: milestone)          project_templates (grain: project)
  tenant_id, id  PK                                tenant_id, project_id PK
  project_id, name, due_on, done_at, position      created_by, created_at
task_milestones      (grain: task — one milestone per task)
  tenant_id, task_id PK, milestone_id
```

Indexes follow the reads: `time_entries (tenant_id, user_id, work_date)`
for the week grid and the submit, `(tenant_id, project_id, work_date)` for
the report, and a partial
`(tenant_id, invoice_id) WHERE invoice_id IS NOT NULL` for the release path
below. `time_weeks (tenant_id, status, week_start)` is the approvals inbox.

### One project list, extended — the decision

**A client project is a `task_projects` row with a `project_clients` row
beside it.** Not a new table, and not new columns on `task_projects`.

*Rejected: a separate `projects` table.* Two project lists in one workspace
is the same failure mode B2.06 refused for to-dos: the board the team
actually opens every morning drifts from the record the invoice is raised
from, and then somebody bills against a project nobody worked in. One list
means a client project's board, its tasks, its labels, its dependencies and
its files are the ones that already work.

*Rejected: adding `customer_id`, `rate_cents`, `budget_*` to
`task_projects`.* That table is owned by `tasks.rs`, whose `create_task_project`
knows nothing about money and should not learn — one file, one reason to
change (law 3). A side table means `tasks.rs` is untouched by this wave,
`project_clients.rs` owns the client facts end to end, and the join is
`LEFT JOIN` — a project without client facts is exactly what an internal
project is, with no sentinel value to misread.

*Rejected: a third `kind` value on `task_projects` (`'client'`).* `kind`
governs **visibility** — `personal` resolves only for its owner, `team` is
tenant-wide — and a third value would quietly answer a question it was not
asked. Instead: **client facts may only be attached to a `team` project**,
and attaching them to a personal board is a `422` naming the rule. An
engagement whose hours are approved by somebody else and billed to a
customer is not private work.

### Bounds, and why these numbers

Reusing `billing_field`'s validators wherever the field is the same field
(`currency`, `country`, `required`, `bounded`, `unit_price_cents`) so a
rate and a price cannot disagree about what a legal amount is.

| Field | Bound | Why |
|---|---|---|
| `minutes` per entry | 1 … 1440 | zero minutes is not work and 24 h is the most a day holds; a night shift over midnight is two entries, one per day, which is also how it must be billed |
| `rate_cents` | ≤ 10⁹ (`UNIT_PRICE_MAX_CENTS`) | the same ceiling a price list line carries, so a rate that becomes a line can never overflow it |
| `budget_minutes` | ≤ 10⁷ | ~19 person-years; beyond that it is a typo |
| `budget_cents` | ≤ 10¹¹ | a billion euro budget, four orders below `i64::MAX` after the arithmetic below |
| `note` | ≤ 500 chars | a note is a sentence about the work, not the deliverable |
| milestones per project | ≤ 200 | a plan a human reads |
| entries per handoff | ≤ 5000 | folds to at most `MAX_LINES` (500) groups; over it → `422`, never a silent truncation |
| tasks copied per template | ≤ 500 | the same restraint, on the copy path |

## The time entry

### Minutes, and the one place they become hours

**Minutes are the stored truth; hours exist only on a document.**
`minutes` is an `i64`-safe integer count, never a decimal, never seconds
(a stopwatch that records to the second invites a UI that bills to the
second, and no European customer wants a line reading 1.0083 h).

Billing's line quantity is **milli-units** (`billing_line::NewLine::qty_milli`,
1.5 = 1500) and the unit we write is `hour`, so somewhere a minute count
becomes a milli-hour count. That conversion is not exact — one minute is
16⅔ milli-hours — so it happens **once, per line, in one pure function**:

```rust
/// Total minutes → the line quantity in milli-hours, rounded to nearest.
/// Integer arithmetic only: 1000/60 = 50/3 = 100/6, +3 to round.
pub fn qty_milli_hours(minutes: i64) -> i64 { (100 * minutes + 3) / 6 }
```

The bound this leaves is stated rather than discovered. A minute count's
exact hour value lands on a fractional part of 0, ⅓ or ⅔ of a milli-hour and
**never on a half** (100 · minutes mod 6 ∈ {0, 2, 4}), so the tie-break never
fires and the error is at most **one third of a milli-hour — 1.2 seconds of
work per line**. The line's money therefore differs from
`minutes × rate / 60` by at most `rate_cents / 3000` cents: **3.3 cents on a
€100/h line**, two thirds of a cent at €20/h. Every third minute (3, 6, 9 …)
is exact, which is why an hour, a quarter-hour and a six-minute stint all
convert without residue at all. Two consequences follow, and both are
design, not accident:

- **The report and the invoice use the same function.** The unbilled view
  and the profitability report fold minutes into money through
  `qty_milli_hours` and then through `billing_totals::totals` — the same
  code the printed invoice uses — so a figure on a report and a figure on a
  document can never disagree by a cent. This is BI1.01's rule ("a chart and
  a tax return cannot disagree") applied one module down.
- **Billing the same work in two instalments may differ by cents from
  billing it once**, because rounding happens per line. That is a property
  of any per-line rounding and is disclosed here rather than treated as a
  bug report later.

*Rejected: a line unit of `minute`.* Exact, and unreadable — an invoice
that says `740 minute` is not a document a client accepts.
*Rejected: adjusting the unit price so the line total is exact.* The unit
price is the agreed rate; a document that misstates the rate to make the
arithmetic tidy is a document that misstates the contract.
*Rejected: two-decimal hours (0.01 h = 36 s), which is what most
time-billing products write.* Our milli-hour is 36× finer at no cost; there
is no reason to be coarser than the schema allows.

No float appears anywhere on this path — not in the store, not in the route
layer, not in the CSV, and not in the browser, which receives cents and
minutes and formats them.

### The running timer is not an entry — the decision

**A running timer lives in `time_timers`, one row per user, keyed by the
user.** Stopping it is what writes a `time_entries` row.

*Rejected: a `time_entries` row with `minutes` NULL.* Two costs, both
permanent. First, every aggregate in the module — the week total, the
report, the unbilled fold, the budget bar — would have to remember to
exclude the running row, and the one that forgets bills a timer that is
still running. Second, "one running timer per user" becomes a rule enforced
by a query instead of by a primary key. With a separate table it is
`PRIMARY KEY (tenant_id, user_id)`: a second concurrent start cannot
represent itself.

**Starting a timer while one runs is a `409`, not an implicit stop.**
*Rejected: stop-and-start in one call*, which is what a phone app does.
Stopping a timer writes a billable fact with a duration; a write nobody
asked for is not a convenience. The UI's one button makes two calls and
both are audited.

A timer carries `billable` and a `note` from the start, so the entry it
writes is complete without a second dialog. `stop` computes minutes as
`ceil` of the elapsed wall-clock — a 30-second stint is one minute, never
zero, because an entry of zero minutes cannot exist (the bound above) and
silently discarding a stint the user started is worse than rounding up 30
seconds. A timer running longer than 24 h stops at 1440 minutes and says so
in the response: somebody went home without stopping it, and the honest
answer is a full day plus a flag, not a 22-hour invoice line.

### The day a person says they worked — the decision

**`work_date` is a `DATE`, supplied by the client in the user's own zone,
and it is what every period boundary uses** — the week, the report, the
unbilled cut-off. `started_at` is kept beside it as provenance when a timer
or a calendar event produced the entry, and is used for nothing else.

*Rejected: deriving the date and the week from `started_at` in UTC.* An
entry stopped at 00:30 in Berlin belongs to the previous working day and
often to the previous *week*; a timesheet whose week boundary moves with
the server's zone is a timesheet an employee will dispute, and they will be
right. A worked day is a calendar fact, not an instant.

Weeks are **ISO 8601 week-numbering weeks, Monday-start**, computed with
`Date::to_iso_week_date()` — the same function `insight_series::bucket_key`
already uses for its week buckets, including its lesson that the
week-numbering *year* is not the calendar year. `week_start` stores the
Monday itself, so no consumer has to recompute it, and a submit route that
is handed a non-Monday is a `422` naming the Monday it should have used.

### Rates: snapshotted, never guessed

`rate_cents` and `currency` are copied onto the entry when it is written,
resolved in one order: the caller's explicit rate → the project's
`rate_cents` → **null**. A later change to the project's rate never rewrites
an entry, for the same reason a price change never rewrites an invoice line
(`billing_line`: a line is a snapshot, not a reference).

**A billable entry with no rate is legal.** The person logging the hour is
frequently not the person who prices it, and refusing the entry would lose
the hour to protect the price. What is *not* legal is billing it: the
handoff (B3.06) demands a rate for every group it raises, exactly as
`crm_handoff` demands a VAT rate rather than guessing one — "a compliance
statement made by a machine" is the phrase that file uses, and pricing is
the same kind of statement. Unrated hours are counted and shown as unrated
in the unbilled view and the report, never dropped and never priced at
zero (the VAT report's honesty rule).

*Rejected for B3: per-person rate cards* (a senior's hour priced above a
junior's). It is real, and it needs the employee record B6.02 builds; adding
`rate_cents` to the resolution order later is additive, and the snapshot
means nothing already written moves.

### Proposed entries are not hours

An entry written by the agent (B3.10) lands `state = 'proposed'` and is
excluded from every aggregate — the week grid's totals, the submit, the
unbilled fold, the report — until a human accepts it, at which point the
rate is resolved and the entry becomes ordinary. This is ADR 0023's rule for
tasks, held to literally rather than approximated: a machine's guess about
somebody's Tuesday is a suggestion, and a suggestion that is invisibly
already in a total is not a suggestion.

## The week: submit, approve, lock

```
(no row) ──submit──> submitted ──approve──> approved ──reopen──> open
    │                    │                                        │
    └────────────────────┴──withdraw/reject──> open / rejected ────┘
                                    (both unlocked, both editable)
```

The lock is **in the store, not in the UI**: creating, editing, moving or
deleting an entry whose `work_date` falls in a `submitted` or `approved`
week is a `409` naming the week. Moving an entry's date checks **both** the
week it leaves and the week it joins, because otherwise a locked week can be
drained one entry at a time.

*Rejected: a `locked` boolean on the entry.* Two places to be right, and a
week reopened would have to rewrite every row it contains. The week's status
is the single fact; the entry's editability is derived from it.

**Reopening an approved week that has billed entries is a `409`** naming
how many are billed and on which invoice. The hours have left the module and
are on a document; the way back is to void or credit that document (B1's
own verbs), not to edit history underneath it.

### Who approves, in B3 — the decision

**A tenant admin approves; the user submits and may withdraw their own
week.** `Account::require_admin` gates approve, reject, reopen and the
inbox — the same gate `/admin/*` uses.

*Rejected: inventing a manager relation now.* "Manager" is a real concept
and it arrives with the employee record and the org chart in **B6.02**, and
the unified approvals inbox in **B6.07**. Deriving a manager from
`task_projects.owner_user_id` would also be wrong on its face: a timesheet
is a person's week and spans several projects, so a per-project owner cannot
approve it. Building half of B6's org chart here to avoid one `require_admin`
is how a permission model gets decided by accident. When B6.02 lands, the
approver check widens additively — admin *or* the submitter's manager — and
nothing already approved moves.

An admin may approve their own week (a one-person tenant has no one else),
and the audit entry records that they did.

## Billable hours become an invoice draft (B3.06)

`POST /projects/invoices` takes a customer, a VAT rate, an optional currency
and a set of entry ids, and returns a **draft** `billing_invoices` id. It
issues nothing, sends nothing, and touches no document it did not just
create — the one-way, one-shot rule `crm_handoff` states for won deals.

Every entry it accepts must be: this tenant's, `state = 'active'`,
`billable`, in an **approved** week, **not already billed**, on a project
whose `project_clients.customer_id` is the customer named, and carrying a
rate whose currency matches the invoice's. Any failure names the count and
the reason (`422`/`409`); the call is all-or-nothing in one transaction, so
a partial invoice with half the hours marked billed cannot exist.

**Grouping: one line per (project, rate).** Description is the project's
name, unit is the word `hour` in the caller's language (the
`?lang=` seam `billing_send::mail_strings_for` and `crm::seed_words_for`
already own — the server writes no untranslated words), quantity is
`qty_milli_hours(Σ minutes)`, unit price is the rate, VAT rate is the one
the caller stated.

*Rejected: one line per entry.* A month of six-minute stints is a
two-hundred-line invoice nobody reads and every client queries.
*Rejected: one line per task.* Tempting, and it is the first thing somebody
will ask for; it is also a per-line rounding multiplier and a disclosure
decision (which task names travel to the customer?). B3 groups by project
and the detail lives in the unbilled view, where the person raising the
invoice can read it. Per-task grouping is named in the out-of-scope list so
its absence is a decision.

Billed entries get `invoice_id` and `billed_at`, and the unbilled view stops
showing them. **The link is released when the document that holds it goes
away**: `delete_billing_invoice` (a draft) and `void_billing_invoice` (an
issued document) each clear `invoice_id`/`billed_at` for their entries in
the same transaction — one additive statement in each, the only place this
wave reaches into B1's code, and an integration test proves the hours return
to unbilled. A **credit note does not release**: crediting is a correction of
a document, the hours stay billed against the original, and re-billing them
would be a second charge for one piece of work.

*Rejected: a foreign key with `ON DELETE SET NULL`.* The FK is composite
(`tenant_id, invoice_id`) and `SET NULL` would null the tenant column too,
which is `NOT NULL`; the column-list form is a newer-Postgres feature this
schema does not otherwise require. A statement inside the existing
transaction is explicit, portable and testable.

## Budgets and the profitability report (B3.08)

`project_clients` carries `budget_minutes`, `budget_cents`, or both, or
neither. Both are **advisory**: logging an hour past the budget is a fact
about the engagement, not an error, and the store never refuses it. The UI
shows a bar and a colour; nothing blocks.

The report answers one question per project per currency for a period:
minutes logged (billable and not), the value of the billable minutes at
their snapshot rates, how much of that is already on a document, the budget,
and the consumption. Money is folded through `qty_milli_hours` +
`billing_totals::totals`, so the report and the invoice agree; entries
without a rate are counted separately and named, never priced.

**Currencies are grouped, never converted** — `crm_report`'s rule, and for
the same reason: adding two currencies with a rate we chose today is an
invented figure. A project's own currency comes from its client facts and a
tenant's accounting currency from `billing_settings.base_currency`; if a
human later wants one restated total, `billing_fx` already knows how to do
it honestly, and that is BI-2's work, not this report's.

CSV follows `billing_reports`: ISO dates, `.` decimals, untranslated column
headers (a file read by a spreadsheet and an accountant's tooling must not
move with the user's locale), and no customer contact data — a project name,
a currency, minutes and amounts.

**This is not cost accounting, and the note says so where the feature is
named.** `docs/features.md` calls it "hours × rates vs budget", which is
exactly what it is: the revenue side. Salary and cost rates — the other half
of the word "profitability" — need the employee record (B6) and the ledger
(B4), and the report's own labels say *value* and *budget*, never *margin*.

## Milestones and templates (B3.09)

A milestone is a named date on a project; a task points at one through
`task_milestones` (PK on `task_id`: one milestone per task, so "which
milestone is this in" has one answer). The link table is this module's,
again so `tasks.rs` gains no column and no reason to change. A milestone is
`done` when a human says so, not when its tasks are — a plan whose dates
move themselves is a plan nobody trusts. The timeline view over the
existing board is a rendering of these rows, not a second model.

A **template** is a project a tenant has marked reusable
(`project_templates`), and instantiating one copies:

- the project row (name from the caller, `kind = 'team'`, colour), and its
  client facts *except* the customer — a template is an engagement shape,
  not a client;
- its tasks: title, description, status, position, priority, labels and
  subtasks;
- its milestones and the task→milestone links.

It copies **no** assignees, comments, activity, followers, attachments, time
entries or dependencies-on-outside-tasks, and it shifts every date by
`starts_on − (the template's earliest milestone date)`: a template with
milestones at day 0, 14 and 30 lands 14 and 30 days after the start date the
caller gave. Task due dates shift by the same delta; a template with no
milestone dates shifts nothing.

*Rejected: a separate template schema (`project_templates` holding a JSON
shape).* A template that is not itself a project cannot be opened, reviewed
or corrected in the UI that already exists, and it drifts from the model it
claims to copy the first time a task gains a field. A template that *is* a
project means the template editor is the board editor.

**As built at B3.11: `due_on` is `NOT NULL`.** The model sketch leaves the
nullability of a milestone's date unsaid, and the migration decided it: a
milestone without a date is not a plan, it is a heading, and a timeline that
has to draw one has nowhere to put it. A deliverable with no date yet belongs
on the board as a task until somebody commits to a day. The consequence is
that the plan's ordering key `(due_on, position)` is total, and "not in the
plan" is a property of a *task* (no `task_milestones` row), never of a
milestone.

## The Projects agent (B3.10)

Three tools in the ADR 0034 allowlist, executed by `alo-jmap` against the
same store functions the routes use — never a parallel path — and verified
**structurally** (routes exist, 401/403/422 guards hold, the execute path
writes the right rows against the local database). No live model call in the
loop, ever.

| Tool | Kind | What it may do |
|---|---|---|
| `log_time` | **draft** | write one `proposed` entry: project (resolved from its name), date, minutes, note. Never `active`, so it is never in a total until accepted |
| `project_status_summary` | **answer** | read hours, budget, milestones and open tasks for one project and answer with them as sources. No writes |
| `draft_timesheet_from_calendar` | **draft** | read the caller's own Agenda events in a range (`calendar::events_in_range`) and propose one entry per event: minutes from the duration, note from the title, `source_kind = 'event'` |

Three rules the third tool needs, because it is the one that touches another
module's data:

- **The project is stated by the caller, never inferred from the event.**
  Guessing which client an event's title belongs to is a billing statement
  made by a machine, and the entry it writes would be pre-approved by
  nobody. One project per call; several calls for several projects.
- **Only the caller's own calendar, through their own `AccountStore`.** An
  agent drafting a timesheet from somebody else's diary is a surveillance
  feature, and the account door makes it unrepresentable.
- **All-day events are skipped and overlaps are flagged, never merged.** A
  day marked "Conference" is not eight billable hours, and two overlapping
  meetings are a question for the human, not an arithmetic problem.

Source resolution ("the Acme project") reuses the B2.10 pattern: match on
the tenant's own project names, exact first then unambiguous prefix, and
**ambiguity is a question back to the user**, never a pick.

**As built at B3.11.** The three tools shipped with those three rules intact
(`agent_projects.rs`, `agent_timesheet.rs`, executed from `agent.rs`'s
allowlist). Two shapes are worth recording because a reader of the table
above would guess otherwise:

- **`draft_timesheet_from_calendar` takes a range, not a day.** `from`/`to`,
  at most 31 days, one proposed entry per *occurrence* — a weekly series
  produces one per week — with `source_kind = "event"` and a source id that
  carries the occurrence's own start, so asking twice never doubles an hour.
  A person filling in a forgotten week asks about the week, not about Tuesday.
- **A refusal is per event, not per batch.** Each occurrence is either drafted
  or left out with a reason code the client has words for — all-day, already
  drafted, no length, longer than a day, that week is submitted, over the
  batch limit, outside the range. The plan is decided before anything is
  written, so a partial batch is the DB-failure path only; the rows it leaves
  are suggestions in nobody's total.
- **A declined meeting is drafted like any other.** The obvious fifth skip
  reason is absent because the store's `attendee_status` is the *organizer's*
  record of guests' replies, not the caller's own RSVP on an invitation they
  received. Modelling the caller's participation is a calendar item, not this
  one; until then, a declined meeting is a suggestion discarded in one click.

## Errors

One map, `billing::map_store_err`, used and not copied — the same call CRM
made (`docs/design/crm.md` § Errors), for the same reason: it is a
store-error map, not a billing rule.

| Condition | Store | Wire |
|---|---|---|
| no or bad token | — | `401` (the `authenticate` extractor) |
| approve/reject/reopen/inbox without admin | — | `403 insufficient role` |
| project, entry, milestone, template or customer not this tenant's | `NotFound` | `404` — existence is never disclosed |
| another user's time entry through the personal door | `NotFound` | `404` — not `403`, which would confirm it exists |
| minutes ≤ 0 or > 1440, note too long, negative budget or rate, unknown currency, bad date, non-Monday week | `Validation(msg)` | `422` naming the rule |
| client facts on a `personal` project | `Validation` | `422` |
| entry's project has no customer, or a different one than the invoice | `Validation` | `422` |
| billable entry with no rate in a handoff | `Validation` | `422`, with the count |
| start a timer while one runs | `Conflict` | `409`, with the running timer in the body |
| stop with no timer running | `NotFound` | `404` |
| write, edit, move or delete an entry in a submitted/approved week | `Conflict` | `409` naming the week |
| submit an already-submitted week; approve a week that is not submitted | `Conflict` | `409` |
| reopen a week with billed entries | `Conflict` | `409` with the count and invoice number |
| entry already billed, or not in an approved week | `Conflict` | `409` |
| > 5000 entries in one handoff, > 500 lines, > 500 tasks in a template copy | `Validation` | `422`, never a silent truncation |
| database error | `Db` | `500`, opaque — the wire never sees a raw error |

Validation messages are authored in the store and name the rule and the
field; they are the one place a message crosses in English today, the
standing cross-cutting item B1.27 and B2.14 both left for a human, and this
wave adds no new kind of it.

## Tenancy

Every statement carries `tenant_id` from the handle, never from request
input — the invariant `for_tenant`/`for_account` make structural rather than
remembered. Two tests are mandatory before B3.03 is done, and one of them is
new to alo's vocabulary:

- **Wrong tenant** (law 1, every wave): tenant A's handle cannot read,
  edit, submit, approve, bill or report on tenant B's entry, week, project
  or template. Clean denial, not data and not a 500.
- **Wrong user** (this module's addition): user B's `AccountStore` cannot
  read, edit, delete, submit or withdraw user A's entries or week — a `404`,
  even inside the same tenant.

### Two doors, deliberately

- **`AccountStore`** owns everything a person does with their own time: the
  timer, their entries, their week, their submit and withdraw. Every
  statement carries `user_id = self.user`, so reaching a colleague's hours
  through this door is unrepresentable in the API, not merely rejected.
- **`TenantStore`** owns the cross-user reads and the decision: the
  approvals inbox, another person's week, the per-user breakdown in the
  report, approve/reject/reopen. The edge gates each of those with
  `require_admin`.

*Rejected: one door, with `AccountStore` functions taking an explicit
`user_id` after an admin check.* That turns "a person's hours are their own"
into a rule every future caller must remember, and the one that forgets
leaks a colleague's diary. Splitting by door means the capability to read
another person's hours is something you must **hold**, and only an admin
route can obtain it — `docs/design/account-scoped-access-door.md`'s doctrine,
applied to the first table where the personal data is *when somebody
worked*.

Client facts, milestones and templates are ordinary tenant-wide business
data on the account door, like `billing_customers`: everyone bills the same
customers and works the same plan.

### The hours of a person are personal data

A record of when an employee worked, on what, and for how long is personal
data under the GDPR and a works-council question in several member states.
The stance, stated here so it is not decided by whichever screen ships
first:

- **Per-user hours are visible to their owner and to a tenant admin, and to
  nobody else.**
- **Project aggregates are visible to anyone who can see the project** —
  hours to date, budget consumption, the value of the work — **without a
  per-person breakdown.** The breakdown is an admin column.
- **Notes never reach a log.** A time note can name a client, a person or a
  case; `tracing` spans carry ids, minute counts and durations, and nothing
  a human typed. The same rule mail bodies have had since Phase 1.
- Every submit, approve, reject, reopen and bill writes one audit entry
  (below), because "who approved my week, and when" is a question an
  employee is entitled to have answered.

### Audit

`projects` joins `audit_action::AUDITED_MODULES` beside `billing` and `crm`
at B3.04 — a one-word additive change, after which `tests/audit_routes.rs`
requires **every** mutating `/projects/*` route to be audited by reading the
router's own source. Sub-resource events file against their parent record
(an approval against the week, a bill against the invoice) so a record's
history is complete, the rule B2.13 established.

## Files this wave added

*As built at B3.11 — the plan is kept where it held, corrected where it did
not.*

Store (`platform/alo-store/src`), one file one reason:

```
project_clients.rs     the client facts of a project, and their validation
project_hours.rs       an engagement's own aggregates (hours to date, billable,
                       billed, consumption) — split out of project_clients.rs
                       when the list read gained a second reason to change
time_entries.rs        CRUD on the caller's own entries + the week lock
time_timer.rs          start/stop, and the entry a stop writes
time_weeks.rs          submit/withdraw (account door) + approve/reject/reopen (tenant door)
time_hours.rs          qty_milli_hours + the fold to money, property-tested
time_invoice.rs        the handoff to billing (B3.06), one transaction
time_report.rs         profitability per project per currency (B3.08)
project_milestones.rs  milestones and the task→milestone link
project_templates.rs   mark, list, instantiate
migrations/0122…0126   project_clients, time_entries, time_timers, time_weeks,
                       project_milestones + task_milestones
migrations/0128        project_templates (0127 went to the sites track, which
                       pushes to the same branch)
```

Routes (`products/mail/alo-jmap/src`): `projects.rs` (the module's own edge
concerns, the language seam for the word `hour`), `projects_clients.rs`,
`projects_time.rs`, `projects_weeks.rs`, `projects_invoices.rs`,
`projects_reports.rs`, `projects_plan.rs` (milestones), `projects_templates.rs`
(templates — a second responsibility that earned its own file), plus
`agent_projects.rs` and `agent_timesheet.rs` (B3.10a/b) and the additive lines
in `server.rs`, `lib.rs` and `audit_action.rs`.

Web (`web/src/projects`): `ProjectsModule.tsx`, `api.ts`, `types.ts`,
`format.ts`, `parts.tsx`, `timerBus.ts`, `ProjectsView.tsx`,
`ClientDialog.tsx`, `WeekView.tsx`, `EntryDialog.tsx`, `PlanView.tsx`,
`MilestoneDialog.tsx`, `TemplateDialog.tsx`, `ApprovalsView.tsx`,
`ReportView.tsx`, `TimerWidget.tsx`, `index.ts`, `format.test.ts`; the
`projects*` block in
`i18n/en.ts`, `fr.ts` and `nl.ts`; the module entry in `product/workplace.tsx`;
`/projects` in `vite.config.ts`. **No `UnbilledView.tsx`** — see § Web surface.

## Out of scope for B3 (cuts are decisions)

- **Cost rates, salaries and true margin** — the other half of
  "profitability". Needs B4's ledger and B6's employees; the report's own
  labels say *value*, never *margin*.
- **Per-person rate cards** — additive to the rate resolution order once
  B6.02 has employees.
- **Per-task invoice lines**, and any per-customer rounding rule (billing in
  15-minute increments). The second is a commercial policy that inflates an
  invoice and must be disclosed on the document a client reads: a human
  decision, not a loop one.
- **Fixed-price engagements and revenue recognition** — a project billed by
  milestone rather than by hour. B3 bills hours; a fixed-price quote is
  already B1's job, and joining them is a wave of its own.
- **Capacity planning, utilisation targets, Gantt with dependencies** —
  `[B+]` in `docs/features.md`, and unchanged by this wave.
- **Automatic time tracking** — app/idle detection, geofencing, anything
  that observes a person rather than recording what they say they did. Not a
  cut for now: a sovereignty product does not ship surveillance.
- **Expenses on a project** (B4.05 links to projects from the other side),
  **project files** (`tasks::project_files` already exists), **CalDAV-driven
  two-way sync** of drafted entries.
- **Per-project access roles** — who may see an engagement at all. The same
  cross-cutting question CRM and Insights deferred, owned by **B4.12** where
  the accountant is the first scoped role. Until then a `team` project and
  its aggregates are tenant-wide, and per-person hours are admin-only, which
  is the narrow half of the answer and the half that protects a person.
- **Invoice-side reporting of billed hours** beyond the entry link (which
  invoice carried which hours is answerable; "revenue by project by month"
  is a chart, and charts are Insights').

## Open questions flagged for a human

- **Should a rejected week notify its owner?** The status is on their screen
  and the decision note with it; a mail on rejection is a product call about
  tone in a European workplace, and the module drafts nothing by itself
  until somebody asks it to.
- **Should an admin be able to log time for someone else?** A manager
  entering a subcontractor's hours is a real request; it is also the exact
  capability the two-door split exists to withhold. Not built, and not
  designed around — when it is asked for it should arrive as an explicit,
  audited, per-tenant setting rather than as a widened door.
- **Does the tenant want minutes or decimal hours on screen?** Minutes are
  stored either way; the grid shows `7:30` in this design because that is
  what a timesheet looks like in the EU. A per-user display preference is
  cheap to add later and not worth a setting nobody asked for now.
- **Compliance, flagged not guessed:** several member states require working
  time to be *recorded* (EU Court of Justice C-55/18) with retention and
  access rules attached, and some require a daily record rather than a
  weekly one. B3 records a day, a person and a duration, which satisfies the
  shape; whether alo claims working-time-record compliance in its marketing
  is a legal statement for a human, not a design decision here.

*As built at B3.11, one of these is answered by the code and three are not.*
**The display question is settled and the design's own answer lost:** the grid
takes `7:30` as an input spelling but *reads back* `7h 30m` in the reader's
language, because a duration beside a total beside a report has to be the same
shape everywhere, and `7:30` beside `2:00` beside a budget bar reads as a
clock. Decimal hours appear nowhere in the interface. The other three — the
rejection notice, logging time for somebody else, and the working-time-record
claim — are untouched and still a human's.

## What B3 promised, and what B3 shipped

Every `[B3]` line of `docs/features.md`, against the code, at the close of the
wave.

| `docs/features.md` | Shipped |
|---|---|
| ★ **Projects agent** — "set up the Acme onboarding project from our template", "what's over budget?", "draft this month's timesheet from my calendar" (draft only — you approve) | **Partly.** Three tools: `log_time` (draft), `project_status_summary` (answer), `draft_timesheet_from_calendar` (draft, a range of days). **Two of the three example sentences are not tools**: setting a project up *from a template* and asking what is over budget *across* engagements. The first is a one-screen action that no user asked a machine for; the second is a portfolio question, and a portfolio question that reads several projects and ranks them is a chart — Insights' shape, not an agent tool's. Named here rather than half-built. |
| Client projects: a project typed as client work (links a customer), budget in hours or money | Shipped (B3.02). Both budgets, either or neither, on the board that already exists. |
| Milestones + simple timeline view over existing task boards | Shipped (B3.09a) as the **Plan** tab: dates on an axis with the board's own tasks under them. `due_on` is required — see § Milestones. |
| Time entry: start/stop timer + manual entry, per task/project, billable flag, hourly rate | Shipped (B3.03, B3.04). The rate is snapshotted onto the entry, so repricing tomorrow never rewrites yesterday. |
| Approval flow: submitted → approved timesheets (weekly), locked after approval | Shipped (B3.05), with withdraw and an audited reopen — and the reopen refuses once hours are on an invoice. |
| ★ Billable hours → invoice lines in one click (feeds B1); unbilled-work view | **API shipped, the click is a cut.** `GET /projects/unbilled` and `POST /projects/invoices` are complete and wire-verified: approved, billable, unbilled hours group the way an invoice groups them and raise a **draft** in Billing. What is missing is the browser screen that selects and raises — the report's *To invoice* column is where the money is visible today. The one feature of this wave a user cannot reach with a mouse. |
| Project profitability: hours × rates vs budget, per project | Shipped (B3.08) with CSV, per currency, never converted. Value, never margin — cost rates need B4's ledger and B6's employees. |
| Project templates (recurring engagement setup) | Shipped (B3.09b). A template *is* a project; instantiating copies the shape and shifts the dates, and copies nobody's assignees, comments, hours or finished cards. |
| `[B+]` Gantt with dependencies; capacity planning; field-service work orders | Out of scope, unchanged. |

Two cross-cutting things this wave did **not** change, restated so a reader
does not assume otherwise: **per-project access roles** are still B4.12's (a
`team` project and its aggregates are tenant-wide; per-person hours are
admin-only), and the **store's validation sentences are still English** — the
standing item B1.27 and B2.14 both left for a human. Everything a browser
renders is en/fr/nl (B3.11), and the one word this module puts on a document a
client reads — the unit label `hour` on an invoice line raised from a
timesheet — has had its own language table since B3.06.
