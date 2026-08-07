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
| `GET/POST /crm/pipelines`, `GET/PATCH /crm/pipelines/{id}`, `POST /crm/pipelines/{id}/archive` | pipeline CRUD (B2.02). *As built (B2.04):* the list route is also what **seeds** a tenant's first board, in the language `?lang=` asks for |
| `GET/POST /crm/pipelines/{id}/stages`, `GET/PATCH/DELETE /crm/stages/{id}`, `POST /crm/stages/{id}/move`, `POST /crm/stages/{id}/archive` | the ordered stage set of a pipeline, with its win/loss flags (B2.02). `DELETE` is for a stage created by mistake — one no deal and no history row has ever named; every other retirement is an archive, because a closed deal must keep pointing at the column it closed in. *As built (B2.04):* `GET /crm/stages/{id}` reads one column, and `POST …/move` is the board drag — its own route, because the rule below says a drag may not rename and an edit may not reorder |
| `GET/POST /crm/deals`, `GET/PATCH/DELETE /crm/deals/{id}` | deal CRUD; list filtered by pipeline, stage, owner, state (B2.03, B2.04) |
| `POST /crm/deals/{id}/stage` | move a deal to a stage (and, on a board, to a position); writes exactly one history row (B2.03) |
| `GET /crm/deals/{id}/history` | the stage history of one deal, oldest first (B2.03) |
| `GET /crm/deals/{id}/threads`, `POST /crm/deals/{id}/threads`, `DELETE /crm/deals/{id}/threads/{threadId}` | the conversations linked to a deal (B2.05). *As built:* the `POST` is **idempotent** — it answers `{"created":false}` for a conversation already linked, because linking twice is the same link |
| `GET /crm/deals/{id}/thread-suggestions[?limit]` | candidate conversations, computed over the **requesting user's own** mail (B2.05). *As built:* `limit` is a page size and is **clamped** (1…50, default 10) rather than refused — it is not an assertion about the data, so the strict-filter rule does not apply to it |
| `GET/POST /crm/deals/{id}/activities`, `DELETE /crm/activities/{id}` | notes and logged calls (B2.06). *As built:* there is no `PATCH` — an activity is written once, and a correction is another note |
| `GET/POST /crm/deals/{id}/next-steps` | the tasks linked to the deal, and the door that creates one (B2.06). *As built:* **plural, with a `GET`**, rather than the `POST …/next-step` this note first wrote. The drawer has to read them back — "shows due in deal" is half the item — and every other CRM collection (`/threads`, `/stages`) is a plural noun carrying both verbs; two spellings of one relationship is a contract nobody can guess |
| `POST /crm/deals/{id}/quote`, `POST /crm/deals/{id}/invoice` | the won-deal handoff to billing: a **draft** quote or invoice for the deal's customer, answering the created document **and the deal** (B2.08) — raising one can give a lead a customer, so the card comes back changed. Body: `{vatRateBp?, country?}`; **not restricted to won deals** (quoting an open deal is how it is won), only a deal recorded as *lost* is refused |
| `GET /crm/reports/pipeline?pipelineId&from&to`, `GET /crm/reports/pipeline.csv?…` | value by stage and win/loss for a period (B2.08). *As built:* **two paths, not a `?format=`** — the shape billing's VAT summary settled on, because a URL that names its representation is the one a browser saves under a sensible name and a script quotes without a query string, and two modules answering "give me the CSV" two different ways is a seam a reader has to remember |
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

*As built (B2.04):* three more conventions the routes inherited from
billing rather than inventing, recorded so the next module does the same.

- **Archiving is its own `POST` with an `{"archived":bool}` body**, and
  an *empty* body means archive — the route's name is already the intent.
  A body that is present but does not state `archived` is a `400`, which
  is billing's contract verbatim (`POST /billing/products/{id}/archive`).
- **`PATCH` is a merge onto the stored record, and the answer carries
  that record.** A field that is not writable — `archived`, `position`,
  `stageId`, `state`, `closedAt` — is ignored exactly as an unknown field
  is, and the caller sees in the answer that it did nothing.
- **The store-error map is billing's** (`alo-jmap/src/billing.rs`), used
  and not copied. It moves to a shared module when a third module needs
  it; renaming a file for no behaviour change is not worth a contract's
  churn.

And one **deviation from the strictness rule above, stated plainly**: the
`ownerUserId` filter is an exact match and is *not* resolved first, so a
user id that owns nothing answers `200` with an empty list rather than
`422`. An owner is a user of the tenant, not a CRM record; the only
tenant-user listing the store publishes is the admin console's
(`TenantStore::list_users`, which carries per-user mailbox usage), and
reaching for it from a sales list would hand every salesperson an
admin-shaped read. `pipelineId` and `stageId` *are* resolved — they are
this module's own records, and both a foreign and an invented id answer
with the same `422`, so the strictness is not an existence oracle.

### Web surface

`web/src/crm`, a rail module of the **workspace product only**
(`product/workplace.tsx`), mounted at `/crm/*` with tabs `board` (where
`/crm` lands) and `list`. It follows billing's three module rules
verbatim — no validation in the client, no money computed in the
browser, an edit sends only what changed — and adds nothing new to the
shell.

The board is the **Tasks board interaction**, not a second one: columns
are stages instead of statuses, a card move is a single-field update,
and the order within a column is the same fractional `position`
(ADR 0022). Reusing the interaction is the point; reusing the *code*
happens only where it stays clean, on the same judgement the billing
line model was shared under.

A deal opens in a drawer, not a page: value and stage at the top,
activities and linked conversations below, with an *open in mail* that
hands off to the mail module rather than rendering a message inside CRM.

*As built (B2.07) — seven things the note left open, decided in code.*

- **The `reports` tab is not here yet.** The pipeline report is B2.08's
  route and lands with it; a tab in front of an endpoint that does not
  exist is a promise, not a surface.
- **No column, and no screen, ever sums money.** A card shows the value
  the server stored for that deal; value-by-stage is B2.08's
  server-computed report, which reports per currency and refuses to
  convert. A browser adding the cards up would answer a different
  question under the same heading.
- **A losing column asks why before the move is sent.** The reason is
  collected in a dialog and the request is made only if it is given;
  cancelling leaves the card where it was and makes no request at all.
  So the `422` the server holds for a reasonless loss is a backstop, not
  the user's experience of it.
- **The open deal lives in `?deal=`**, not in component state, so a link
  to a deal is a link a colleague can be sent. It survives a reload and
  a tab switch, and the drawer **re-reads the deal** rather than
  rendering the row the board is holding.
- **The list's filters are the server's; the search box is not, and says
  so.** Column, owner and state go into the query (the strictness above
  is what makes the selects safe to build from the server's own answer);
  the text box plainly matches the rows already on screen. The owner
  filter sends the signed-in user's OIDC subject, which *is* the
  tenant's user id (`alo-identity/src/oauth.rs`).
- **`/mail?thread=<id>` is a new, additive mail deep link** beside the
  existing `?open=<messageId>` a task uses: CRM knows a conversation and
  not any one message in it. The mail module resolves the thread through
  the *reading* user's own account door, so CRM hands over an id and
  never a right to read it — which is why "open in mail" is offered only
  where the server's computed `readable` is true, and a colleague who
  does not hold the conversation is told who linked it instead.
- **A deal in an archived column names it as one.** A closed deal can
  sit in a column that was archived afterwards; the drawer's stage
  select shows "Archived column" rather than falling back to its first
  option, which would show the wrong column and turn an idle click into
  a move.

*As built (B2.08) — closing a deal, and the report the tab now has.*

- **The `reports` tab exists now**, as the third one, and it is the only
  CRM screen that shows a total. It renders **two tables per currency**,
  not one: the open board by stage, and what closed in the period. They
  answer different questions — the first is a snapshot as it stands, the
  second is bounded by the two days — and one table would invite reading
  a column across both.
- **The lost reason is a picker with a free-text field**, not a plain
  prompt and not a closed list. The store takes any string, so a fixed
  vocabulary here would be a rule invented by a screen; but a reason
  nobody can answer in one click is a reason nobody enters. The
  suggestions fill the field, and the field is what is sent.
- **The handoff is offered from the deal drawer**, on any deal that is
  not lost, as *Quote* and *Invoice*. It asks for exactly what the deal
  cannot answer — the VAT rate of its line, and the country of a
  customer about to be created from a lead — and nothing else; the
  question is skipped entirely when the deal makes it unnecessary. It
  shows the server's total and then hands off to `/billing/…/{id}`
  rather than rendering a document a second time.
- **`saveTextFile` moved from `web/src/billing` to `web/src/platform`.**
  Saving a fetched export is a browser mechanism, not a module's rule,
  and the pipeline report is its second caller — moving it was cheaper
  and truer than a copy. `formatRate`, `quarterOf` and
  `previousQuarterOf` stayed in billing and are read through its public
  `index.ts`, because a quarter and a rate *are* rules billing owns.

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
  form. *As built (B2.02):* a tenant's **active** boards carry distinct
  names — a partial unique index on `(tenant_id, lower(name)) WHERE
  archived_at IS NULL`. Two tabs called "Sales" mean nothing to the
  person reading them, and that same uniqueness is what makes the
  first-use seed race-free without a lock (below). An archived board
  frees its name; restoring it into an occupied name is the `409` the
  error map carries.
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
  `closed_at`), `created_by`, timestamps. *As built (B2.03):* the snapshot
  is whole or absent (a `CHECK` ties `outcome` to `closed_at`) and a lost
  reason exists exactly when the outcome is `lost` — the rule that makes
  "lost reasons + win/loss reporting" a report rather than a wish. The
  value ceiling, the currency shape and a non-blank title are `CHECK`s too:
  the store validates all of them first, so a violation there is a bug in
  our code rather than bad user input.
- **`crm_deal_stage_events`** — append-only: deal ref, `from_stage_id`
  (NULL for the row written at creation), `to_stage_id`, `moved_by`,
  `moved_at`. Written in the **same transaction** as the move. *As built
  (B2.03):* its id type is `CrmEventId`; both stage foreign keys are
  `RESTRICT`, so a column the past has named cannot be deleted even by a
  caller who bypasses the store, and the rows go with the deal (`CASCADE`)
  and with the tenant.
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

### What the report's period actually bounds (B2.08)

**The period bounds the outcomes, and not the stage rows.** Won and lost
are the deals whose `closed_at` falls inside `[from, to]` — the snapshot
frozen on each of them, so re-flagging a column next year never rewrites
last year's win rate. Value by stage is the **open board as it stands
now**, unfiltered, and the answer says so out loud (`openAsOf`).

**Rejected: applying the period to the open rows too.** A column has no
history to date a deal by. The stage events say when a deal moved, so
"what stood in Proposal on 31 March" is answerable — but it is a
reconstruction over a different table with its own edge cases (a deal
raised and closed inside the period, a column archived since), and it is
a different report. Pretending the period applied here would put a
figure under a heading it does not belong to, which is the one thing a
number on a manager's screen must not do.

The **win rate is counted by deals, not by value**, and it is basis
points — an integer, like every other ratio in alo. "We win one in
three" is the sentence a sales team acts on; a value-weighted rate
swings on one large deal. Over no closed deals it is `null`, not zero: a
rate over nothing is unanswered, and a client that drew `0` there would
be saying "we lost everything".

### Winning a deal raises a draft, and never more (B2.08)

`POST /crm/deals/{id}/quote|invoice` is the one place CRM writes into
billing, and three rules keep that seam narrow.

- **It raises a draft.** No number is consumed from the gapless
  sequence, nothing is issued, nothing is sent. What happens to the
  document afterwards is billing's, through billing's own routes.
- **A lead becomes a customer, once.** A deal that already names a
  `billing_customers` row bills that row; one that does not creates it
  from the deal's own company/contact fields and **writes it back onto
  the deal**, so a second document bills the same company rather than a
  twin of it. The company name is required — naming a customer after the
  *opportunity* ("Renewal — Acme GmbH") would put a sentence where a
  legal name belongs on every document that follows — and the country is
  asked for, because it decides VAT treatment and is the one fact a
  customer needs that a deal does not carry.
- **The VAT rate is stated by the caller, never guessed.** A deal
  carries one number and a line needs a rate; choosing one on the
  tenant's behalf would be a compliance statement made by a machine. So
  a priced deal demands `vatRateBp` and a deal worth nothing raises an
  **empty** draft — a header for the right customer that a human prices,
  rather than a line worth zero, which would be a zero-rated supply
  nobody meant to declare.

**Not restricted to won deals.** Quoting an open deal is ordinary sales
— the quote is often how it is won — so the only lifecycle rule here is
that a deal somebody has recorded as *lost* is not a thing to invoice
for. Reopening it makes it billable again, as a reopen makes everything
else about it true again.

**Known, and deliberate:** billing's writes are not transactional with
the deal's, so two callers racing the handoff on the same lead can leave
one unused customer row behind (the link is written only while the deal
still has none, so the loser bills the winner's customer rather than
overwriting it). Holding a lock across another module's writes to avoid
an archivable empty row would be the worse trade.

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

*As built (B2.04) — the open question is answered:* the language is the
one the **client making that first read** asks for, with `?lang=` on
`GET /crm/pipelines`, falling back to English for a tag we do not ship.
It is the only language anybody is actually looking at, and the words are
ordinary user data from the moment they are written, so a tenant that
disagrees renames them. The tables (en/fr/nl) live at the route edge in
`alo-jmap/src/crm.rs`, the same seam the covering emails of
`billing_send.rs` use; the store still only writes the names it is
handed. `?lang=` on any later read does nothing at all.

*As built (B2.02):* the seed writes the board and all its columns in
**one transaction**, so a tenant is never left holding a board with half
its columns, and two colleagues opening the module in the same instant
get **one** board: the loser of the race hits the active-name
uniqueness, swallows it, and reads back what the winner wrote. Seeding
is a first-use rule, not an every-read one — a tenant that archived its
only board is not handed a new one the next morning. A column's *place*
on the board moves through its own store call (`move_crm_stage`),
separate from the edit that renames it or changes its flags: a board
drag must not be able to rename a column, and saving an edit form must
not be able to reorder the board.

## Moving a deal, and the history of it

A move is one `POST` that, in a single transaction: re-reads the deal
under its row lock, checks the target stage belongs to the **same
pipeline** (`Validation`, `422` — a board is not a place to lose a deal
into another team's funnel), writes `stage_id` and `position`, writes
the closing snapshot if the target stage is flagged, and appends exactly
one `crm_deal_stage_events` row. Creating a deal writes the same event
with `from_stage_id = NULL`, so "how long did this sit in Qualified" is
answerable from row one, not from row two.

*As built (B2.03):* four things the note left open, decided in code and
recorded here rather than left to be rediscovered.

- **A reposition inside one column writes no event.** A history row saying
  Qualified → Qualified answers no question and would spoil every velocity
  figure computed from these rows, so the event is appended only when the
  column actually changed. Dragging a card up its own column still writes
  the position, in the same call.
- **An archived column takes no new cards.** Moving a deal into one — or
  creating a deal in one — is a `Validation` (`422`), which is what
  "archived" was defined to mean when the column was archived.
- **Moves and shape changes are serialised on the board row.** Creating or
  moving a deal takes the pipeline row `FOR SHARE`; adding, archiving or
  deleting a column, and archiving the board, take it `FOR UPDATE`. Card
  moves therefore never block each other, and no card can slip into a column
  between the count that finds it empty and the archive that hides it.
- **A deal is deleted, not archived** — the one CRM record that is. It is our
  own private note of an opportunity, not a document anybody else holds, so
  one raised by mistake leaves no trace (its history goes with it). A deal
  that was really worked is *lost*, which is a move.

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

*As built (B2.05) — five things the note left open, decided in code.*

- **A conversation is threaded per user, so a link is per copy.**
  `AccountStore::resolve_thread` matches references against
  `(tenant, user)`, so two colleagues on one email hold **two** thread
  rows. `readable` is therefore true for the linker and for anyone whose
  messages are in that thread row (delegated access to the same account
  door), and false for a colleague holding their own copy — who can link
  *their* copy to the same tenant-wide deal, and reads it back as their
  own. Nothing in the note changes; it is the concrete shape of "reads
  resolve through the reading user's door", and the one that makes the
  computed `readable` flag necessary rather than decorative.
- **The subject a non-holder sees is `threads.subject_base`** — the
  normalised, lower-cased, `Re:`-stripped label. A reader who *does* hold
  the conversation sees the subject of their own newest message in it.
  One field crosses, and it crosses as the least of itself.
- **Correspondents, not just senders.** The queue said "pure fn over
  message from-addrs"; the matcher reads `From` **and** `To`, because a
  sales thread is usually one *we* started and a from-only rule would miss
  most of a pipeline. Deliberate, recorded, and one argument to
  `crm_thread_match::match_message` if a human disagrees.
- **The scan is bounded and so is the link count.** A suggestion pass
  reads the requesting user's 500 most recent messages
  (`SUGGESTION_SCAN_MESSAGES`) and answers at most 50
  (`SUGGESTIONS_MAX`, default 10); a deal holds at most 100 conversations
  (`DEAL_THREADS_MAX`, the note's cap), enforced under the deal's row lock
  so two colleagues linking at once cannot walk past it. Conversations
  already linked are left out of the suggestions — proposing them again is
  noise.
- **Unlinking is open to the whole tenant, linking is not.** Writing a
  link needs the linker's own door; removing one needs only tenant
  membership, because a link left by a colleague who has since left the
  company would otherwise be permanent, and removing it destroys nothing —
  the link never held the mail.

*As built (B2.05) — the database backstop.* `threads` is keyed on its id
alone, which is enough to point at a thread but not at a thread *of this
tenant*, so migration `0114` adds a unique index on
`threads (tenant_id, id)` and points `crm_deal_threads` at it with a
composite foreign key. The per-user rule stays in code, where it belongs;
the coarser tenant rule is now enforced by Postgres as well.

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

*As built (B2.06) — six things the note left open, decided in code.*

- **The log is bounded per deal, not paged** (`DEAL_ACTIVITIES_MAX`, 500),
  enforced under the deal's row lock exactly as the conversation cap is. The
  drawer reads the log whole, so the read is bounded by the record rather than
  by a cursor nobody would page; a deal that has collected five hundred notes
  has stopped being one opportunity. A note is ≤ 10 000 characters, as the
  Bounds section already said.
- **`happened_at` is an instant, and it is not `created_at`.** A call logged an
  hour later is dated the hour it took place, and the log is ordered by *when it
  happened*; the row still records when it was entered. The route parses it as
  full RFC 3339 (`parse_rfc3339`, the deliberate opposite of the `YYYY-MM-DD`
  rule a deal's `expectedClose` lives under) and normalises to UTC.
- **`kind` is a closed vocabulary** (`note` | `call` | `meeting`) with a `CHECK`
  behind it: an unrecognised word is a `422` and never a silent `note`, because
  a log that quietly demotes a call to a note is worse than one that refuses the
  word.
- **A next step's *source* is written by us, never by the caller.** Whatever a
  request says about `sourceKind`/`sourceId`/`state` is overwritten with this
  deal, `active`: a "next step" that points somewhere else is not one, and a
  person clicking is not the agent proposing (ADR 0023 stays in Tasks).
- **A next step is only as visible as the task it is.** Reading a tenant-wide
  deal does not widen a personal project by one row: a colleague sees the next
  steps on team projects plus anything assigned to them. It is the same
  asymmetry a linked conversation has, and the read applies the tasks module's
  own visibility rule rather than a second one CRM invented
  (`AccountStore::tasks_for_source`, which the mail surface can reuse for
  `source_kind = 'email'`).
- **Deleting a deal deletes its log and leaves its next steps standing.** The
  activities are the deal's own rows and cascade with it; the tasks are the
  *user's* rows, and a task must not vanish out of somebody's morning list
  because a salesperson tidied up a board. Their source link then points at a
  deal that is gone — which is what an ADR 0021 source link has always been: a
  pointer that may not resolve, never a foreign key.

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
| A lost reason sent for a stage that is not `is_lost` (as built, B2.03) | `Validation` | `422` |
| Moving a deal into, or creating one in, an **archived** stage (as built, B2.03) | `Validation` | `422` |
| Naming an owner who is not a user of this tenant (as built, B2.03) | `Validation` | `422` |
| A deal position that is not a finite number (as built, B2.03) | `Validation` | `422` |
| Naming a customer that is absent, archived, or another tenant's | `NotFound` / `Validation` | `404` / `422` |
| A stage flagged both won and lost, or a second won/lost stage in one pipeline | `Validation` | `422` |
| Creating or renaming a pipeline onto the name of another **active** pipeline (as built, B2.02) | `Conflict` | `409` |
| More stages on one pipeline than the cap allows (as built, B2.02: 200) | `Validation` | `422` |
| A stage position that is not a finite number (as built, B2.02) | `Validation` | `422` |
| Deleting the last remaining stage of a pipeline | `Conflict` | `409` |
| Deleting a stage any deal or history row has ever named (archive it instead; as built, B2.03) | `Conflict` | `409` |
| Archiving a stage that still holds open deals (as built, B2.03) | `Conflict` | `409` |
| Archiving a pipeline that still has open deals (as built, B2.03) | `Conflict` | `409` |
| Deleting an activity written by somebody else | `Forbidden` | `403` |
| An activity with a blank body, or one over 10 000 characters (as built, B2.06) | `Validation` | `422` |
| An activity `kind` that is not note/call/meeting (as built, B2.06) | — (route edge) | `422` |
| `happenedAt` / `dueAt` that is not a full RFC 3339 instant (as built, B2.06) | — (route edge) | `422` |
| More activities on one deal than the cap allows (as built, B2.06: 500) | `Conflict` | `409` |
| A next step with a blank title (as built, B2.06) | — (route edge) | `422` |
| A next step filed on a project the caller cannot see (as built, B2.06) | `NotFound` | `404` |
| Linking a thread that is absent, another tenant's, **or one the requesting user has no message in** | `NotFound` | `404` |
| Linking a thread already linked to this deal | — | `200`, idempotent (`created:false`) |
| Linking beyond the per-deal thread cap (as built, B2.05: 100) | `Conflict` | `409` |
| Linking without stating a `threadId` (as built, B2.05) | — (route edge) | `422` |
| Unlinking a link that is absent or another deal's | `NotFound` | `404` |
| Suggestions for a deal with no usable address (as built, B2.05) | — | `200`, empty |
| Listing with a `state` filter that is not one of open/won/lost | — (route edge) | `422` |
| Listing with a `pipelineId`/`stageId` filter this tenant does not have (as built, B2.04) | — (route edge) | `422` |
| Listing with an `ownerUserId` who owns nothing (as built, B2.04) | — | `200`, empty |
| A move (`POST …/stage`, `POST /crm/stages/{id}/move`) that does not say where (as built, B2.04) | — (route edge) | `422` |
| Creating a deal without a `pipelineId` **or** without a `stageId` (as built, B2.04) | — (route edge) | `422` |
| Report `from`/`to` malformed or absent, or a report with no `pipelineId` (as built, B2.08) | — (route edge) | `422` |
| Report `from` after `to` (as built, B2.08) | `Validation` | `422` |
| Raising a document from a deal recorded as **lost** (as built, B2.08) | `Validation` | `422` |
| Raising a document from a **priced** deal without stating `vatRateBp` (as built, B2.08) | `Validation` | `422` |
| Raising a document from a lead with no `companyName`, or with no valid two-letter `country` (as built, B2.08) | `Validation` | `422` |
| Raising a document from a deal worth more than one line may carry (as built, B2.08: 10^9 cents) | `Validation` | `422` |
| Import file that is not readable CSV, has no header row, or exceeds the row cap | `Validation` | `422` |
| Import commit where any row is invalid (all-or-nothing) | `Validation` | `422` + the per-row report |

The wrong-tenant case returns the **same `404`** as a truly absent id:
no existence oracle across tenants, the doctrine documented in
`platform/alo-store/src/error.rs` and followed by every billing route.

The four rows that count **open deals** or **history rows** were deferred
from B2.02 — a guard written against a table that does not exist is a
guess, not defence in depth — and **landed with B2.03**, in the item that
created the table they count. Each is enforced under the board's row lock,
so a concurrent card move cannot walk past it, and each has a database
backstop behind it (`RESTRICT` on both stage foreign keys).

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
