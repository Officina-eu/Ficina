# ADR 0017 — Send-as identities and mailbox delegation

Status: accepted (send-as, shared mailboxes, and delegation all shipped)

## Update — shared-mailbox delegation shipped

The design below was subsequently built (migrations 0030–0031). What landed:

- **Grant table** `account_delegates(tenant_id, owner_id, delegate_id,
  can_write, send_mode)` — tenant-scoped, `owner_id <> delegate_id`,
  `send_mode ∈ {none, as, on_behalf}`; a send mode implies write.
- **Access door** — `resolve_target` in `alo-jmap` turns each request's
  `accountId` into either the signed-in user's own account or a delegated
  mailbox they hold a grant on **in the same tenant**; no grant reads as
  `accountNotFound` (no oracle). A read-only delegate is refused every `…/set`
  (`accountReadOnly`); a delegate without a send grant is refused submission
  (`forbiddenToSend`).
- **Send-as vs on-behalf** — `as` sends `From:` the owner; `on_behalf` also
  prepends a `Sender:` of the acting delegate.
- **Self-service** — a user can share their own mailbox (`/jmap/delegates`)
  without an admin, Gmail-style; admins manage any mailbox's delegates.
- **Acceptance gate met** — isolation tests first: cross-tenant grants
  invisible, ungranted access is `accountNotFound`, read-only can't mutate,
  no-send can't send, revocation immediate.
- **Per-folder delegation** (migration 0032, `delegate_folders`) — a grant may
  optionally confine a delegate to specific folders. Enforced on `Mailbox/get`
  (granted folders only; others `notFound`, no oracle), `Email/get`/`query`
  (only messages in granted folders), `Email/set` (create/move only into granted
  folders; flag/destroy only granted messages), and `Mailbox/set` (restructuring
  refused). Isolation test is the gate; fails closed on a store error. Granting
  a folder implicitly grants its **subfolders** — `resolve_target` expands the
  raw grant to its descendant closure before enforcement.
- **Always-mounted + live** — every accessible mailbox is mounted in the sidebar
  at once (not a switcher), and changes made by one delegate reach the others in
  real time over the push stream (owner-account `StateChange`, delegate streams
  subscribe to their granted owners). A grant added/revoked mid-session is
  **instant**: grant/revoke publishes a `TYPE_DELEGATION` signal to the
  delegate's own stream, which re-evaluates its subscription set in place (no
  reconnect) and prompts the client to re-list shared mailboxes.

The original design and rationale follow.


## Context

Two related capabilities are wanted for parity with Microsoft 365 / Gmail:

- **Send-as** — send a message from one of the addresses a user already owns
  (their canonical address or an alias), choosing the From in compose.
- **Shared mailboxes + send-on-behalf delegation** — let one user read and act
  in *another* principal's mailbox (a team `support@` box, a manager's inbox a
  PA manages), and send either **as** that address or **on behalf of** it
  (`From:` the shared address, `Sender:` the acting user).

These differ enormously in blast radius. Send-as is authorization over a user's
*own* addresses; delegation is **cross-account access**, which is exactly what
Law #1 ("the tenant is sacred; isolation is tested, not assumed") is most
sensitive about. Conflating them would smuggle a new access-control model in
behind a UI change.

### What already exists (verified)

- Aliases: `aliases(address → one user_id)` (migration 0008), with
  `add_alias` / `remove_alias` / `aliases_of` and an admin UI.
- The submission path (`alo-jmap` `submission.rs`) already authorizes an
  outgoing message by requiring **both** the `From:` header and the envelope
  `mailFrom` to be in `{ canonical } ∪ aliases_of(user)` — else `forbiddenFrom`.
  So a user is *already* permitted to send from any of their own addresses.
- `AccountStore` is scoped to exactly one `(tenant, user)`; every mailbox,
  message, and keyword read/write goes through that door. There is **no** ACL,
  delegation, "act-as", or `Sender:` support anywhere. Groups are fan-out
  distribution lists (a copy per member's inbox), not shared mailboxes.

## Decision

**Ship send-as now; specify delegation here and implement it as its own
isolation-gated effort.**

### Send-as (done)

- The session resource advertises `alo:sendAs` — the addresses the signed-in
  user may send from (`email_of` ∪ `aliases_of`), computed the same way the
  submission path authorizes them, so the picker can never offer an address the
  server would reject.
- Compose shows a From picker when the user holds more than one address; the
  chosen address flows into the draft's `From` and the submission envelope.
- No new authorization surface: the existing `submission.rs` check is the sole
  gate, and it already covered exactly this set.

### Shared-mailbox delegation (designed, deferred)

The design when built:

- **Grant model** — a new `mailbox_delegates(tenant_id, owner_user_id,
  delegate_user_id, may_send_as, may_send_on_behalf)` table, tenant-scoped,
  admin-managed. A grant is strictly within one tenant; a cross-tenant grant is
  impossible by construction.
- **Access path** — a delegate opens the owner's mailbox through a new
  *explicitly authorized* `for_delegated_account(owner, acting_delegate)` door
  that checks the grant on every call. `AccountStore`'s single-user invariant is
  preserved for the default path; delegation is an additional, audited door —
  never a relaxation of the existing one.
- **Send-as vs on-behalf** — `may_send_as` extends the submission `valid` set to
  include the owner's address (`From:` owner, no `Sender:`). `may_send_on_behalf`
  emits a `Sender:` header (the acting user) alongside `From:` the owner — which
  requires adding a `sender` field to the MIME builder (`mime.rs`), absent today.
- **Tests first** — isolation tests are the acceptance gate: a delegate with no
  grant is `NotFound` on the owner's mailbox (indistinguishable from a foreign
  id); a grant in tenant A never reaches tenant B; revocation is immediate.

Deferred because it is a new cross-account access model touching the most
security-critical invariant in the system, and it deserves its own reviewed
change with the isolation tests written first — not an end-of-batch addition.

## Consequences

- Users can pick a From among their own addresses today, with zero new
  authorization surface.
- Shared mailboxes and on-behalf sending have a written design and a clear,
  test-first path; no partial or unsafe delegation code ships in the meantime.
- When built, delegation adds one grant table, one audited access door, a
  `Sender:` field in the MIME builder, and an admin UI for grants.
