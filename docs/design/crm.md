# Design note — alo CRM (pipelines, deals, and the mail they live on)

Status: **design** (written ahead of the first migration) · 2026-08-07 ·
ADR 0035 · Business track wave B2

alo CRM is the second Work OS module: the opportunity → deal → won arc
for EU SMEs, built on the same tenant-scoped store as alo Billing. Its
one structural advantage over every standalone CRM is that the mail is
already here — a deal does not need a plugin to see the conversation it
came from, because the conversation and the deal are rows in the same
database, under the same tenant. That advantage is also the sharpest
privacy question in the module, so it gets its own section and its own
rejected alternative.

This note records the surface, the data model, the error map, the
tenancy rules, and the three decisions worth arguing about (what a
pipeline is scoped to, what a thread link *is*, and where a deal's next
step lives) before the first migration lands. Sections marked *as built*
describe code that exists; on the day this file is written, none do.

> **Wave gate, flagged for a human.** `ROADMAP.md` gates wave B2 on
> "B1 live with ≥1 real tenant", and B1 is code-complete but not
> deployed. This note is design work, which is exactly what belongs
> ahead of an unmet gate; the first B2 **migration** (B2.02) is the point
> where a human should confirm the gate or move it. Recorded in
> `docs/autonomy/STATE.md` rather than decided here.

## Surface

- **Inputs:** authenticated workspace users driving `/crm/*` routes on
  `alo-jmap` — pipeline and stage administration, deal CRUD, the stage
  move, thread links, activities, the lead import, and the pipeline
  report. The CRM agent (ADR 0034, item B2.10) is a second caller of the
  same store functions, never of a parallel code path.
- **Outputs:** JSON resources; CSV for the pipeline report and the
  import report; mail **drafts** (never sends) when a follow-up goes to
  a contact; and real Tasks in the existing tasks module when a deal
  gets a next step.
- **Who calls it:** `web/src/crm` (the module UI, B2.07) calls
  `alo-jmap`; the `alo-ai` CRM module produces propose-then-approve
  envelopes that `alo-jmap` executes. Nothing external calls CRM.

`/crm` is a **new top-level route prefix**: the production Caddyfile
needs it added at the next deploy, the same standing human action
`/billing` carries. Noted in STATE.md at B2.04, not touched by the loop.

### Routes

All under the authenticated `alo-jmap` router, following the convention
`/billing/*` established (typed `Problem` errors, the `authenticate`
extractor, registration in `server.rs`, and the store-error map in
`billing.rs` — see "Errors" for what CRM shares and what it adds).

| Route | Purpose |
|---|---|
| `GET/POST /crm/pipelines`, `GET/PATCH /crm/pipelines/{id}`, `POST /crm/pipelines/{id}/archive` | pipeline CRUD (B2.02) |
| `GET/POST /crm/pipelines/{id}/stages`, `PATCH/DELETE /crm/stages/{id}`, `POST /crm/stages/{id}/archive` | the ordered stage set of a pipeline, with its win/loss flags (B2.02). `DELETE` is for a stage created by mistake — one no deal and no history row has ever named; every other retirement is an archive, because a closed deal must keep pointing at the column it closed in |
| `GET/POST /crm/deals`, `GET/PATCH/DELETE /crm/deals/{id}` | deal CRUD; list filtered by pipeline, stage, owner, state (B2.03, B2.04) |
| `POST /crm/deals/{id}/stage` | move a deal to a stage (and, on a board, to a position); writes exactly one history row (B2.03) |
| `GET /crm/deals/{id}/history` | the stage history of one deal, oldest first (B2.03) |
| `GET /crm/deals/{id}/threads`, `POST /crm/deals/{id}/threads`, `DELETE /crm/deals/{id}/threads/{threadId}` | the conversations linked to a deal (B2.05) |
| `GET /crm/deals/{id}/thread-suggestions` | candidate conversations, computed over the **requesting user's own** mail (B2.05) |
| `GET/POST /crm/deals/{id}/activities`, `DELETE /crm/activities/{id}` | notes and logged calls (B2.06) |
| `POST /crm/deals/{id}/next-step` | create a Task in the tasks module, linked back to the deal (B2.06) |
| `POST /crm/deals/{id}/quote`, `POST /crm/deals/{id}/invoice` | the won-deal handoff to billing: a **draft** quote or invoice for the deal's customer, answering the created document (B2.08) |
| `GET /crm/reports/pipeline?pipelineId&from&to[&format=csv]` | value by stage and win/loss for a period (B2.08) |
| `POST /crm/imports/leads/preview`, `POST /crm/imports/leads` | CSV mapping preview, then the commit (B2.09) |

Five conventions the CRM routes hold themselves to, so the surface
cannot drift from billing's:

- **A lifecycle change is its own `POST`, never a field on the
  `PATCH`.** Moving a deal to a stage writes history and can close the
  deal; it must not happen because an editor submitted a stale form.
  `stageId`, `outcome`, `closedAt` and `position` are therefore not
  writable by `PATCH`, and like any unknown field they are ignored.
- **Money is only ever written as integer cents and read back
  computed.** The deal's `valueCents` is what a user typed (in cents);
  every *sum* — value by stage, forecast, won total — is computed
  server-side and never in the browser.
- **`state` is derived on read** from the deal's snapshotted outcome, in
  the same spirit as billing's `overdue`: `open`, `won`, `lost`.
- **Filters are strict.** An unrecognised `state`, `stage` or `owner`
  filter is a `422`, not a silently widened list — a sales manager
  reading "everything" when they asked for "mine" is a wrong number on a
  screen, which is worse than an error.
- **No route echoes mail content that the caller could not already
  read.** See "Deal ↔ mail thread".

### Web surface (planned, B2.07)

`web/src/crm`, a rail module of the **workspace product only**
(`product/workplace.tsx`), mounted at `/crm/*` with tabs `deals` (the
board, where `/crm` lands), `list`, and `reports`. It follows billing's
three module rules verbatim — no validation in the client, no money
computed in the browser, an edit sends only what changed — and adds
nothing new to the shell.

The board is the **Tasks board interaction**, not a second one: columns
are stages instead of statuses, a card move is a single-field update,
and the order within a column is the same fractional `position`
(ADR 0022). Reusing the interaction is the point; reusing the *code*
happens only where it stays clean, on the same judgement the billing
line model was shared under.

A deal opens in a drawer, not a page: value and stage at the top,
activities and linked conversations below, with an *open in mail* that
hands off to the mail module rather than rendering a message inside CRM.

## Data model

New `crm_*` store modules in `platform/alo-store` (one file per
responsibility, mirroring `billing_*` and `tasks.rs`). Every table
carries `tenant_id` and cascades with the tenant, ids are `opaque_id!`
newtypes (`CrmPipelineId`, `CrmStageId`, `CrmDealId`,
`CrmActivityId`), timestamps are `timestamptz`, dates that mean a **day**
are `date`, and **money is `i64` integer cents**. No floating point
appears in any money column, struct field, or computation — the only
`double precision` in the module is the board `position`, which is an
ordering, not a quantity.

Migrations continue the business track's `01xx` block (`0112_…`
onwards); the sites track continues in `00xx`.

- **`crm_pipelines`** — name, optional description, `archived_at`
  (archive, never delete: a closed deal must always be able to name the
  pipeline it was won in), `created_by`, timestamps. A tenant's first
  pipeline and its stages are **seeded on first use** (see below), so a
  new tenant opens the module onto a working board rather than a setup
  form.
- **`crm_stages`** — pipeline ref, name, `position` (fractional, like a
  task's), `is_won`, `is_lost`, `archived_at`. The two flags are what
  make a column mean "closed"; a stage may set at most one of them
  (CHECK), and a pipeline may hold at most one stage of each kind
  (partial unique index) — a board with two "Won" columns has no
  win rate.
- **`crm_deals`** — pipeline ref, stage ref, title, the customer link
  and the lead fields (below), `value_cents`, `currency`,
  `expected_close` (a date), `owner_user_id`, `source`, `position`
  within the stage, the closing snapshot (`outcome`, `lost_reason`,
  `closed_at`), `created_by`, timestamps.
- **`crm_deal_stage_events`** — append-only: deal ref, `from_stage_id`
  (NULL for the row written at creation), `to_stage_id`, `moved_by`,
  `moved_at`. Written in the **same transaction** as the move.
- **`crm_deal_threads`** — deal ref, `thread_id`, `linked_by`,
  `linked_at`, unique on `(tenant_id, deal_id, thread_id)`. (The queue
  calls this table `deal_threads`; it takes the module prefix every
  other table in alo carries.)
- **`crm_activities`** — deal ref, `kind` (`note` | `call` | `meeting`),
  body, `happened_at`, `author_user_id`, `created_at`. A **next step is
  not a row here** — see "Activities and next steps".

### The customer, the lead, and the contact

A deal names a **`billing_customers` row** when the company is already
one the tenant invoices, and carries `company_name`, `contact_name` and
`contact_email` as its own columns when it is still a lead. Winning a
deal that has no customer row creates one (B2.08) from exactly those
fields, which is why they are shaped like the customer's.

**Rejected: a CRM-owned "organisation" table.** A customer record is
already the tenant's record of a company (B1.02), and a second one
guarantees two spellings of the same company, two VAT ids, and a merge
problem the day someone invoices the deal. CRM extends the owner rather
than growing a sibling that half-overlaps it.

The optional `contact_id` deserves a warning that the code will repeat:
**contacts are per-user** (`contacts.user_id` — they are address books
synced over CardDAV), while a deal is tenant-wide. A link to a contact
therefore resolves only for the colleague who owns that address-book
entry. That is why the name and email the whole sales team must see are
**columns on the deal**, and `contact_id` is a convenience pointer that
may simply not resolve for a reader — never an error, never a blank
deal. `billing_customers.contact_id` already carries this asymmetry; CRM
inherits it deliberately rather than discovering it in a bug report.

### Bounds, and why these numbers

- `value_cents`: `0 ≤ v ≤ 10^11` (one billion euro). It is an `i64`-safe
  ceiling no SME deal reaches, and the pipeline report sums thousands of
  them: 10^11 × 10^4 deals is 10^15, comfortably inside `i64`. A
  negative deal value is not a discount, it is a typo.
- `currency`: ISO 4217, validated by `billing_field::currency` — the
  same function, so a currency cannot be legal in one module and not the
  other.
- `title` ≤ 200 chars, `lost_reason` ≤ 200, activity body ≤ 10 000,
  `source` ≤ 60, ≤ 200 stages per pipeline, ≤ 100 linked threads per
  deal. Every one of them is a `Validation` naming the rule.

### The pipeline report never converts currencies

Value by stage is reported **grouped by currency**, and a mixed-currency
pipeline yields one row per currency rather than one converted total.
**Rejected: converting the forecast to the accounting currency** at
B1.21's stored rates. Those rates are snapshotted at *issue* on a
document that exists; a forecast has no issue date, so converting it
would mean picking today's rate for money that may arrive next quarter —
a number that changes when nobody changed anything and reconciles
against nothing. A tenant who wants one number can ask for it after the
deal is invoiced, where a real rate exists.

## Pipelines and stages — the decision

**Chosen: a pipeline is tenant-wide, and a tenant may have several.**
"Per-team" in the queue is satisfied by *several pipelines per tenant* —
New Business, Renewals, one per sales team — distinguished by name and
listed in full to every member of the tenant. There is no per-pipeline
access boundary in B2.

**Rejected: scoping a pipeline to a Space (ADR 0026) now.** Role-based
access per module is a real, listed `[B2]` feature in
`docs/features.md`, and it is *cross-cutting* — finance, sales and HR
worlds are one mechanism, not three. Building half of it here (a
nullable `space_id` that a later item has to reinterpret) would settle
that design by accident, from the narrowest of its five callers. When
the role item lands it attaches to pipelines additively, with its own
migration adding the column and its own tests; until then the honest
statement is that **every member of a tenant sees every deal**, which is
also how most SME sales teams actually work.

**Also rejected: per-user pipelines**, the shape `tasks` uses for
personal projects. A pipeline only one person can see defeats the record
— a deal is a company asset, and the reason CRMs exist at all is that
the person who owns the deal is sometimes not the person who has to
answer for it.

**Seeding.** On a tenant's first read of the module, one pipeline
("Sales") is created with five stages — New, Qualified, Proposal, Won
(`is_won`), Lost (`is_lost`) — by the same upsert that reads them, so a
tenant that never opens CRM has no rows at all. Stage *names* are seeded
in the tenant's language through the i18n catalogue at the route edge,
not hardcoded English in the store: the store is handed the names it
should write. Stage names are user data from that moment on — renaming
"Qualified" is a rename, not a schema change, which is exactly why the
board's meaning lives in the two flags and not in the names.

## Moving a deal, and the history of it

A move is one `POST` that, in a single transaction: re-reads the deal
under its row lock, checks the target stage belongs to the **same
pipeline** (`Validation`, `422` — a board is not a place to lose a deal
into another team's funnel), writes `stage_id` and `position`, writes
the closing snapshot if the target stage is flagged, and appends exactly
one `crm_deal_stage_events` row. Creating a deal writes the same event
with `from_stage_id = NULL`, so "how long did this sit in Qualified" is
answerable from row one, not from row two.

**Rejected: deriving stage history from the audit log (B2.13).** The
audit log is administrative, best-effort by design (an audit failure
must never fail the primary action — `platform/alo-store/src/audit.rs`),
and its detail is a free-text string. Funnel and velocity reporting
needs rows that are typed, transactional, and guaranteed present. Both
exist and neither replaces the other: the audit log answers "who changed
this record", the stage events answer "what did this deal do".

### Won, lost, and reopened

The stage flags decide what a *move* means; the deal's own `outcome`,
`lost_reason` and `closed_at` record what it *was*. Moving into a
flagged stage writes that snapshot in the same transaction, so
re-flagging a stage next year never rewrites last year's win rate — the
same reason a billing line snapshots its price instead of joining to the
price list.

Moving into a stage flagged `is_lost` **requires a lost reason**
(`Validation`, `422`): "Lost reasons + simple win/loss reporting" is the
feature, and a reason that is optional is a reason nobody enters.

Moving a closed deal back to an open stage is **allowed**, and clears
the snapshot while leaving both events in the history. This is a
deliberate contrast with a quote's terminal states (B1.11): a quote is a
document the customer holds, so a change of mind is a new quote; a deal
is our own private record of an opportunity, and pretending it cannot
reopen just produces a second deal for the same customer and a win rate
counted twice.

## Deal ↔ mail thread — the decision

This is the module's reason to exist, and the place where a careless
design would quietly turn a private mailbox into a shared one.

**What a link is:** one row saying *this deal and this conversation
belong together*, written only by a user who can already see the
conversation, only when they confirm it. It stores the thread's id, who
linked it, and when. **It stores no message content — not a body, not a
participant list, not a count.**

**What a link is not:** a copy. Mail stays in mail. Every read of a
linked conversation resolves through **the reading user's own account
door** (`AccountStore::thread_messages`, which is scoped to
`(tenant, user)` because `messages.user_id` is per-user), so:

- a colleague who has the thread in their own mailbox sees it and can
  open it in mail;
- a colleague who does not sees **that a conversation is linked, its
  subject, and who linked it**, and cannot open it.

The subject is the one field that crosses, and it crosses knowingly:
`threads.subject_base` is a tenant-scoped row by construction, and
linking is a deliberate act of sharing by a user who could have written
the same subject into a note. Bodies, addresses and message counts never
cross a mailbox boundary at all. Where a reader cannot open a link, the
UI says who linked it — the useful answer is "ask Sam", not a silent
gap.

**Linking requires the thread to resolve through the linker's own
door.** A thread the requesting user has no message in answers `404`,
identical to a thread that does not exist — no existence oracle, the
same doctrine the wrong-tenant `404` follows. So a user cannot attach a
conversation they have never seen by guessing an id.

**Suggestion is a pure function, and it never links anything.**
`suggest_threads` takes the deal's customer/contact email addresses and
a page of the requesting user's own recent messages, and scores
candidates: an exact address match first, then a **domain** match. Two
rules keep it honest:

- **Free-mail domains never match by domain.** `gmail.com`,
  `outlook.com`, `hotmail.com`, `yahoo.*`, `proton.me` and their
  siblings are carried in a small constant list; for those, only the
  full address matches. Half of European SME customers mail from Gmail,
  and domain-matching there would suggest every personal message the
  user has.
- **A suggestion is a proposal, exactly like an AI one** (ADR 0023's
  posture, applied to a heuristic): it appears as a candidate with the
  reason it matched, and becomes a link only on an explicit `POST`.

**Rejected: automatic linking on a domain match.** It is the obvious
feature and it is wrong twice. A customer with three deals would have
every conversation attached to all three, and a tenant whose customer
uses a shared free-mail domain would find private mail attached to a
record the whole company reads. The `[B2]` feature line says
"automatically … (same-domain matching, **user-confirmed**)"; the
confirmation is the feature, not the friction.

**Also rejected: copying the messages into a CRM activity feed** — the
shape most CRMs use. It duplicates content into a table with different
tenancy from the mail store, ages instantly, and makes deleting a
message a two-place problem. The unfair advantage here is that we do not
need the copy.

## Activities and next steps

Notes and logged calls are `crm_activities` rows: a kind, a body, when
it happened, who wrote it. They are written once and deleted only by
their author (`Forbidden`, `403`, for anybody else — the record is
readable tenant-wide, so hiding the row's existence would be theatre),
and they are never mail. There is no edit: a correction is another note,
which is what a log of what was said and done ought to be.

A **next step is a Task**, created in the existing tasks store with
`source_kind = 'deal'` and `source_id = <deal id>` — the additive third
value alongside `email` and `event` (ADR 0021's source-link pattern,
which exists precisely for this). The deal drawer shows its open tasks
by reading them back through that link.

**Rejected: a `next_step` column (or a CRM-private to-do table) on the
deal.** Two to-do lists in one workspace is how a CRM becomes the system
nobody updates: the task that matters ends up in the list the user
actually opens every morning, and the CRM's copy rots. The task lands in
a project the user picks — defaulting to their personal project, because
the next step belongs to the person who will do it.

## Importing leads (B2.09)

`POST /crm/imports/leads/preview` takes an uploaded file and a column
mapping, and answers a **report**: the rows that would be created, the
rows that would be skipped as duplicates (matched on contact email, then
on the email **domain** of an existing customer or open deal), and the
rows that cannot be imported with the rule each one broke. Nothing is
written.

`POST /crm/imports/leads` commits, **all-or-nothing in one
transaction**. A partial import leaves a user guessing which half
landed and re-importing to find out; the preview already named every
blocking row, so refusing the whole file costs one fix and one retry.
Skipped duplicates are not failures — they are reported and the import
proceeds.

**CSV only** (RFC 4180, the same dialect `alo-jmap/src/csv.rs` writes).
`.xlsx` is a ZIP of XML parts and a new dependency; it is listed in
Out of scope below with that reason, and every spreadsheet in Europe
exports CSV.

## The CRM agent (B2.10)

Three tools on ADR 0034's propose-then-approve envelope, executed by
`alo-jmap` against the same store functions the routes call:
`create_deal` (including from a thread the user is reading, which
carries the link through as a *proposed* link), `move_deal_stage`, and
`draft_followup` — which drafts a mail into the user's Drafts and never
sends it, the same rule the billing agent lives under and the same
absolute rail the loop lives under.

Verification in the loop is **structural**: the routes exist, the guards
answer `401`/`422`, and the executors run against the local database.
No model is called; wiring one is a human step.

## Errors

Store errors are `StoreError` variants (`thiserror`), mapped at the
route edge to the existing `Problem` shape by the map in
`alo-jmap/src/billing.rs` — CRM reuses that function rather than writing
a second one that drifts (it is a store-error map, not a billing rule;
it moves to a shared module the moment a third caller needs it, which is
this one).

| Condition | Store | HTTP |
|---|---|---|
| Unauthenticated request | — | `401` |
| Pipeline/stage/deal/activity id absent **or owned by another tenant** | `NotFound` | `404` |
| Deal value negative or above the ceiling; unknown currency | `Validation` | `422` |
| `expectedClose` that is not exactly `YYYY-MM-DD` | — (route edge) | `422` |
| Title blank, or any bounded field over its limit | `Validation` | `422` |
| Creating a deal without naming a pipeline and stage | — (route edge) | `422` |
| Moving a deal to a stage of a **different pipeline** | `Validation` | `422` |
| Moving a deal into an `is_lost` stage without a reason | `Validation` | `422` |
| Naming a customer that is absent, archived, or another tenant's | `NotFound` / `Validation` | `404` / `422` |
| A stage flagged both won and lost, or a second won/lost stage in one pipeline | `Validation` | `422` |
| Deleting the last remaining stage of a pipeline | `Conflict` | `409` |
| Deleting a stage any deal or history row has ever named (archive it instead) | `Conflict` | `409` |
| Archiving a stage that still holds open deals | `Conflict` | `409` |
| Archiving a pipeline that still has open deals | `Conflict` | `409` |
| Deleting an activity written by somebody else | `Forbidden` | `403` |
| Linking a thread that is absent, another tenant's, **or one the requesting user has no message in** | `NotFound` | `404` |
| Linking a thread already linked to this deal | — | `200`, idempotent (unique row) |
| Linking beyond the per-deal thread cap | `Conflict` | `409` |
| Unlinking a link that is absent or another deal's | `NotFound` | `404` |
| Listing with a `state`/`stage`/`owner` filter that is not recognised | — (route edge) | `422` |
| Report `from`/`to` malformed, or `from` after `to` | — (route edge) | `422` |
| Import file that is not readable CSV, has no header row, or exceeds the row cap | `Validation` | `422` |
| Import commit where any row is invalid (all-or-nothing) | `Validation` | `422` + the per-row report |

The wrong-tenant case returns the **same `404`** as a truly absent id:
no existence oracle across tenants, the doctrine documented in
`platform/alo-store/src/error.rs` and followed by every billing route.

## Tenancy

Every `crm_*` table carries `tenant_id` with `REFERENCES tenants (id) ON
DELETE CASCADE`, and every read and write goes through
`Store::for_account(tenant, user)` — the `AccountStore` door that bakes
`(tenant, user)` into the query rather than accepting a tenant argument
a caller could get wrong. No CRM function takes a `TenantId` parameter;
the handle is the scope.

Concretely:

- Every `SELECT`, `UPDATE` and `DELETE` includes `tenant_id = $1` from
  the handle, never from request input.
- Foreign keys are validated **within the tenant**: a deal's stage is
  re-resolved under the same handle (and against the deal's pipeline), a
  deal's customer likewise, so a guessed id from another tenant is a
  `404`, not a cross-tenant link.
- **The thread link is the one place tenancy is not the whole story**,
  because mail is scoped tighter than the tenant: a thread row is
  tenant-scoped, its messages are per-user. Writing a link requires the
  thread to resolve through the *linker's* account door; reading a
  linked conversation's messages always goes through the *reader's*.
  Neither path ever accepts a thread id as authority for what it can
  show.
- CRM records are **tenant-wide reads** by design (see "Pipelines and
  stages"), so the isolation boundary this module defends is the tenant,
  and the mailbox boundary is defended separately and explicitly above.
- **Every B2 storage item ships a wrong-tenant test** (mandatory per
  CLAUDE.md and LOOP.md): tenant A reaching tenant B's pipeline, stage,
  deal, activity and — the one that matters most — tenant B's *thread*
  each gets a clean denial, proven by a test. B2.05's test asserts
  specifically that a thread of another tenant can never be linked, and
  that a thread of another **user of the same tenant** cannot be linked
  by someone who does not hold it.

## What else wave B2 carries

Three queue items in this wave are not CRM and are not documented here,
so this note stays one file with one reason to change:

- **B2.11 recurring invoices** and **B2.12 SEPA pain.001 export** are
  billing extensions; their design lands in `docs/design/billing.md`
  where the invoice model already is.
- **B2.13 the audit log** is cross-cutting (billing *and* CRM, and every
  module after). It extends the existing `audit.rs` spine and gets its
  own note when it is built, not a section inside a module note.

## Out of scope for B2

Deliberate cuts, each a decision rather than an omission:

- **Per-pipeline / per-role access control** — the cross-cutting
  `[B2]` roles-on-Spaces feature, deliberately not half-built here
  (see "Pipelines and stages"). Until it lands, every member of a tenant
  sees every deal, and this note says so out loud.
- **`.xlsx` import** — CSV at full depth in B2.09; a ZIP-of-XML parser
  and its dependency are their own decision.
- **Email tracking (opens, clicks)** — a tracking pixel in a sovereignty
  product is a contradiction, and ADR 0035's positioning rules it out.
- **Automatic sending of any email** — `draft_followup` and every other
  mail path creates Drafts a human approves, consistent with ADR 0034
  and the loop's absolute no-real-email rail.
- **Lead scoring / forecasting models** — an AI judgement about a
  person's likelihood to buy, which needs a written EU AI Act posture
  before it needs code.
- **Marketing campaigns, sequences, mass sends** — a different product;
  `[B+]` at best.
- **Live AI model calls in the loop** — B2.10 is verified structurally,
  as B1.25 was.
- **Merging duplicate deals or customers** — the import *skips*
  duplicates rather than merging them; a merge tool is a real item once
  there is real data to merge.

## Open questions flagged for a human

1. **The B2 wave gate** (`ROADMAP.md`): B1 is not live with a real
   tenant. Confirm or move the gate before B2.02's migration.
2. **Whose language seeds the stage names** when the first user of a
   tenant to open CRM is not the tenant's admin. The note's answer is
   "the requesting user's", which is right for a solo tenant and
   arguable for a mixed-language team; renaming is a rename, so the cost
   of being wrong is small.
3. **Whether a linked conversation should be openable by a colleague who
   does not hold it** — i.e. whether CRM should eventually ask mail for
   a *shared* view of a linked thread. That is a delegation feature with
   its own consent model, and it is not B2.
