# The audit trail

The cross-cutting record of **who changed what, and when**, for the business
modules (ADR 0035). Written by wave B2.13, and the note `docs/design/crm.md`
promised would exist when it was built rather than a section inside one module's
note — because the trail belongs to no module and every module after CRM will
join it without touching this design.

The question it answers is asked from a record, not from a console: a
bookkeeper looking at an invoice whose total is not what they remember, a sales
manager looking at a deal that moved back a column. So the trail is addressed by
record, shown on the record, and readable by the people who work on the record.

## The surface

| Route | What |
|---|---|
| `GET /audit?entity=<type>:<id>&limit=<n>` | one record's history, newest first |
| `GET /admin/audit` (existing) | the tenant's whole log, admin-only |

`/audit` is a **new top-level prefix**: the production Caddyfile needs it added
at the next deploy, alongside `/billing` and `/crm`. The vite dev proxy has it.

`entity` is one parameter carrying a pair — `billing.invoice:4f2c…` — because it
is one address; half of it is never useful. The type is validated against the
shape the vocabulary uses (lowercase `module.record`) before it reaches a query.

**`401`** without a bearer token. **`422`** when `entity` is missing or is not a
`type:id` pair. **Never `404`**: an id from another tenant, and an id that never
existed, both answer `200` with an empty list. The endpoint is not an oracle for
which ids are real.

`/audit` is deliberately **not** admin-gated. It is the history of a record the
caller can already open and edit; hiding "who changed this invoice" from the
people working on it would only push them to ask each other in chat. The admin
console keeps the tenant-wide view, which now shows business events alongside
administrative ones — one log, two ways to read it.

## The data model

One table, `audit_log`, the one migration 0015 already created for
administrative actions. Migration 0118 adds two nullable columns:

- `entity_type` — a stable dotted name for the kind of record
  (`billing.invoice`, `crm.deal`);
- `entity_id` — the record's own id within the tenant.

Both `NULL` on every existing row, which is exactly right: an administrative
action's subject is the `target` label, not a record.

**Rejected: a second table.** A separate `business_audit` would have been
cleaner to index and would have kept `/admin/audit` unchanged. It was rejected
because an audit trail that lives in two tables is an audit trail with two
answers — and the first question anyone asks it ("everything that happened in
this tenant, in order") would have needed a `UNION` for ever after.

An entry carries an actor, a verb, a record, the route, and a time. It carries
**no before/after values and no request body**. A log that quotes what changed is
a second copy of the record, kept somewhere with different access rules — the
exact leak a sovereignty product cannot afford (Law 1). "Who and when" is the
question; "what did it say before" is a different feature with a different
design.

### Append-only

There is no `UPDATE` or `DELETE` path to `audit_log` anywhere in the codebase:
`platform/alo-store/src/audit.rs` is the whole surface, and it writes and reads.
Rows leave only with the tenant (`ON DELETE CASCADE`, 0015). This is a property
of the code, so it is tested rather than declared in DDL — a database-level
guarantee needs a role split this deployment does not have yet, and is worth
revisiting when it does.

## The vocabulary, and why it is derived

An action is `<module>.<record>.<verb>`: `billing.invoice.issue`,
`crm.deal.activity.create`. It is derived from the **matched route** by
`products/mail/alo-jmap/src/audit_action.rs`, not written by hand in the handler.

**Rejected: each handler recording its own entry.** It reads better at each call
site, and it is what the existing administrative log does. It was rejected
because this item's promise is *every* mutating route writes *exactly one*
entry — a promise kept fifty times by hand is kept until the fifty-first route,
whose author has no reason to know the promise exists. Deriving from the route
makes coverage a property of the router: a new `POST /billing/…` is audited the
moment it is registered.

The cost is that the vocabulary is mechanical rather than chosen, and that a
sub-resource event is filed against its **parent** record (a payment on its
invoice, a note on its deal) rather than against itself. Both turned out to be
the right side of the trade: filing on the parent is what makes a record's
history complete, and `tests/audit_routes.rs` pins the whole derived vocabulary
as a golden list, so a route that produces a badly-reading action fails a test
instead of shipping.

Two rules bound what is recorded:

- **Only successes.** A refused write changed nothing; a history that lists
  rejected edits reads as a record of changes that were never made.
- **Only writes.** Reads are never recorded, and the one `POST` that mutates
  nothing — the lead-import dry run — is on an explicit, tested exception list.

## Tenancy

`list_entity_audit` is on `TenantStore` and its `WHERE` names the tenant, so a
record id is only ever resolved inside the caller's own tenant. `entity_id` is
an id from another table and nothing at the database level ties it to a tenant,
so two tenants holding the same id string is the case that matters —
`platform/alo-store/tests/audit_trail_tenancy.rs` writes exactly that collision
and proves each tenant sees only its own entry.

The write side takes the actor from the bearer token, never from a body.

## Out of scope

Deliberate cuts, each a decision rather than an omission:

- **Field-level diffs** — see above; a different feature, and one that changes
  what the log is allowed to contain.
- **Retention and export** — entries live as long as the tenant. A retention
  policy and a "download the log" button are a compliance item with its own
  legal reading, not a side effect of this one.
- **Mail, calendar, drive and chat** — each already records change its own way,
  and folding them in would make "the audit log" mean something different in
  each module. The layer is scoped to `/billing` and `/crm` by an explicit list.
- **Agent-executed actions** are recorded when they go through a billing or CRM
  route, and not when an executor calls the store directly (ADR 0034's own
  proposal/approval record is where those live today). Unifying the two is worth
  its own item.
- **A tenant-wide business feed** — "everything that happened today" exists in
  the admin console's log; a filtered, paged, module-aware version of it belongs
  with alo Insights (ADR 0037), not here.
