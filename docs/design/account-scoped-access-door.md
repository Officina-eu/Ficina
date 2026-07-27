# The account-scoped access door

`ficina-store` makes **tenancy structural**: mail data is reachable only
through a `TenantStore`, obtained via `Store::for_tenant`, and every
query bakes `tenant_id = $tenant`. This note extends the same discipline
one level down — to the **account** (a JMAP account is a user) — so that
cross-*account* access within a tenant is unrepresentable in the API,
not merely guarded by a call a caller must remember to make.

## The problem this closes

Before this change, per-user data (mailboxes, messages, threads,
keywords, blobs, the change log) lived on `TenantStore`, and every
method that took an id relied on the *caller* first invoking an
ownership guard — `owns_message`, `owns_mailbox`, `owns_thread`,
`owns_blob` — to reject another user's id. That is the exact shape of
the tenancy bug we already refused: **a guard you can forget**. The
account-isolation lesson in `docs/interop.md` says the boundary must be
enforced by construction, so a wrong id returns `NotFound` in the data
with no guard in the path to omit.

## The door

`Store::for_account(TenantId, UserId) -> AccountStore` is a pure, no-I/O
constructor (mirroring `for_tenant`). `AccountStore` holds `(tenant,
user)` privately and bakes **both** into every statement it issues. No
`AccountStore` method accepts a tenant or user argument. Holding an
`AccountStore` for the wrong `(tenant, user)` yields empty results /
`NotFound` from every query, because no rows match its predicate — you
cannot even *name* another account's rows without its `AccountStore`.

- `messages` and `mailboxes` carry `user_id`, so account scoping is a
  direct `AND user_id = $user` on those tables.
- `threads` and `blobs` are tenant-level by schema (threads group mail
  across a tenant; blobs are deduplicated per tenant, `refcount`-GC'd),
  and have **no** `user_id`. Per-account access to them is mediated
  through owned messages: an `AccountStore` read of a thread or blob
  joins `messages WHERE user_id = $user`, so a user only ever reaches a
  thread they have a message in, or a blob one of their messages
  references. The old `owns_thread`/`owns_blob` logic becomes the WHERE
  clause of the read itself.
- `object_changes` gained `user_id` (migration `0004`), so `/changes`
  is a per-account range scan. The state string remains the tenant-wide
  monotonic modseq (a shared, always-increasing counter); `/changes`
  filters by `user_id`, so an account never sees another account's
  deltas. Keeping one tenant counter avoids a second sequence and is
  correct for JMAP (state need only be monotonic; deltas are filtered).

`TenantStore` keeps **only genuinely tenant-level** operations:
`create_user`, `user_by_email`, `set_credentials` (provisioning and
login-key management within a tenant) and `tenant()`. Everything that
touches one user's owned rows moves to `AccountStore`, and the
`owns_*` guards are **deleted** — there is nothing left to forget.

## Delivery and APPEND

Writing a user's message (SMTP delivery, and later IMAP `APPEND`) means
resolving the recipient user and obtaining their `AccountStore`, then
calling `ingest`/`deliver` on it — one ingestion path, one write, no
second parser. The account scope is therefore enforced on the write
side too: you cannot ingest into a user's account without holding that
account's door.

## Isolation proof

The isolation suite is extended so that, for **every** `AccountStore`
path and every JMAP method, an id belonging to account B, addressed
through account A's door, returns `NotFound`/empty — never data, never a
`500` — with **no ownership guard anywhere in the call path**. The proof
is now "the type you hold is the boundary," not "the guard you called."

## Rejected alternatives

- **Keep the `owns_*` guards on `TenantStore`.** Rejected: it is a
  forgettable check, the precise failure mode we already eliminated for
  tenancy. One missed guard on one new method is a cross-account leak.
- **Row-Level Security (Postgres RLS) with a per-request `SET`.** Real
  defense in depth, but it depends on session-local GUCs surviving
  pool-connection reuse and does not make the *Rust* API safe on its
  own — a bug could still issue an unscoped query that RLS quietly
  filters, masking the error. We want the mistake to be
  *uncompilable/unrepresentable*, not silently corrected. RLS remains a
  worthwhile future belt-and-braces, tracked separately, not a
  substitute for the typed door.
- **A single `for_account` that also verifies the user exists (I/O).**
  Rejected: `for_tenant` is pure and so is this. A non-existent account
  simply matches no rows; making the constructor do a round-trip would
  add latency and an error path without improving isolation (the
  queries already return `NotFound`).
- **One `Store` method per object taking `(tenant, user, id)`.**
  Rejected: it puts the scoping ids back in every call site — exactly
  what invites a caller to pass the wrong one. The whole point is that
  the ids are held once, by the handle, and never passed again.
