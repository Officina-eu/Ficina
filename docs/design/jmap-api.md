# JMAP API (design note)

`alo-jmap` is alo's native client protocol (RFC 8620 core, RFC
8621 mail) — an HTTP service over `alo-store`. **From merge it is a
public contract:** the web app, desktop cache, and compat adapters speak
it, so every surface here changes *additively* forever (CLAUDE.md
"contracts outlive code").

## Surface & transport (RFC 8620 §2)

- `GET /.well-known/jmap` → the **Session** resource: `capabilities`
  (`urn:ietf:params:jmap:core`, `urn:ietf:params:jmap:mail`),
  `accounts`, `primaryAccounts`, `apiUrl`, `downloadUrl`, `uploadUrl`,
  `eventSourceUrl`, `state`. Limits are honest and enforced:
  `maxSizeUpload` 50 MiB, `maxConcurrentUpload` 4, `maxSizeRequestObject`
  10 MiB, `maxCallsInRequest` 32, `maxObjectsInGet` 500,
  `maxObjectsInSet` 500, `maxObjectsInQuery`/`collationAlgorithms` per
  spec.
- `POST /jmap/api` → a **Request** `{ using, methodCalls, createdIds? }`
  → **Response** `{ methodResponses, sessionState, createdIds }`.
  Method calls run in order; a method's typed error becomes an error
  *invocation* (`["error", {type,...}, callId]`), request-level problems
  (bad JSON, unknown capability, over a limit) are a `400` problem
  object per §3.6.1. `resultReferences` (back-references) are supported
  for chaining.
- Errors are **exact**: request-level `{ type: "urn:ietf:params:jmap:
  error:*" }`, method-level `{ type }` from §3.6.2 (`invalidArguments`,
  `serverFail`, `notFound`, `cannotCalculateChanges`, `stateMismatch`,
  `overQuota`, …). Internal detail never leaks into `description`.

## Interim auth (b)

Identity (OIDC) is a later milestone, so auth is deliberately minimal
and **behind a trait** (`Authenticator`) the future `alo-identity`
implements without touching method code:

- `POST /auth/token` `{ username, password }` → verifies an **argon2**
  hash in the store's `credentials` table → issues an opaque bearer
  token (random, stored only as a SHA-256 hash in `api_tokens`) → `{
  token, accountId }`.
- Every API/blob/event request carries `Authorization: Bearer <token>`;
  the token resolves to `(TenantId, UserId)` and the handler enters the
  store **only** through `Store::for_tenant(tenant)`. The tenant claim
  is never taken from the request body.

**Rejected — session cookies / a full OAuth2 flow now.** Rejected: it
is identity-server theater we would rebuild against `alo-identity`
anyway, and a stateful cookie session is a second auth model to migrate.
A bearer token that maps to `(tenant, account)` is the smallest thing
that (a) exercises the real tenant door and (b) swaps cleanly for
OIDC-issued bearer tokens later — same header, same resolution trait,
only the issuer changes.

## Change tracking / state tokens (e) — the store addendum

JMAP `/changes` needs monotonic state. We add to the **store** (this is
the sanctioned store change; per Law 3 it lives in a new
`alo-store::changes` module, not bolted onto `store.rs`):

- `tenant_modseq(tenant_id, modseq BIGINT)` — a per-tenant monotonic
  counter, bumped once per mutating transaction.
- `object_changes(tenant_id, type, id, created_modseq, modseq,
  destroyed)` — one row per object, upserted on every create/update/
  destroy to the new modseq (destroyed keeps a tombstone). `/changes`
  since state `S`: rows with `modseq > S`, split into created
  (`created_modseq > S`, live), updated (`created_modseq ≤ S`, live),
  destroyed (`destroyed`), bounded by `maxObjectsInGet`; `newState` is
  the current tenant modseq. A caller state older than the compaction
  horizon gets `cannotCalculateChanges` honestly (we keep full history
  for now, so this fires only on an unpar8seable/garbage token).
- State tokens are the decimal modseq as an **opaque** string — clients
  must not parse them.

**Rejected — a per-object version column with no change log.** A bare
`version` per row cannot answer "what was *destroyed* since S" (the row
is gone) and cannot distinguish created from updated without scanning
every row. The `object_changes` tombstone log answers all three of
`created`/`updated`/`destroyed` in one indexed range scan on
`(tenant_id, type, modseq)`, which is exactly `/changes`'s shape.

**Rejected — reusing Postgres LSN / logical replication as the state.**
It is not per-tenant, leaks a global cursor across tenants, and couples
our public state token to a Postgres internal. The per-tenant modseq is
ours, monotonic, and isolation-safe.

## Mailbox (c) & Email (d)

`Mailbox/{get,set,changes}` and `Email/{get,query,set,changes}` +
`Thread/get`, mapped onto store calls. Mailbox counters come straight
from the store's transactional `total`/`unread` — never recomputed in
the API. `Email/query` filters (`inMailbox`, `from`/`to`/`subject`/
`text`, `before`/`after`, `hasKeyword`) map to store queries; sort is
`receivedAt`; `position`/`anchor`/`limit` paginate with the store's
`Page` caps. `Email/get` returns spec properties from the parsed stored
message, with `bodyValues` (text parts) truncated to `maxBodyValueBytes`
and `isTruncated` set.

## Blobs (f) — one id space

`POST {uploadUrl}` streams into the store's content-addressed blob layer
with the size ceiling enforced **during read** (protocol rule); the
returned `blobId` **is** the store's blob id — no second id space.
`GET {downloadUrl}/{blobId}/{name}` is tenant-scoped (the blob is read
only under the caller's tenant prefix), serves the stored `Content-Type`
with `Content-Disposition: attachment` and `X-Content-Type-Options:
nosniff` (no sniffing).

## Push (g)

`GET {eventSourceUrl}` is a `text/event-stream` emitting `StateChange`
for `Mailbox`/`Email`/`Thread`, plus heartbeat comments. Each connection
is authenticated to one tenant and subscribes to a **per-tenant**
broadcast; a tenant's stream is structurally silent about other tenants
(isolation surface — tested).

## Tenancy

Every method, both blob endpoints, and every event stream reach data
only via `for_tenant(claim)`. The wrong-tenant suite extends to all of
them: tenant A wielding B's ids/blobIds/state tokens gets a clean
`notFound`/empty, never data, never a 500.

## Out of scope (recorded)

`EmailSubmission/set` (h) is the sanctioned cut seam if the session runs
long. Also out of scope this milestone: full MIME attachment trees in
`Email/get` (we return text `bodyValues` + `hasAttachment`; the full
`bodyStructure` is additive later), JMAP-over-WebSocket (RFC 8887), and
`SearchSnippet`/`Identity`/`VacationResponse`.
