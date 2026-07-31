# IMAP + POP3 compatibility shims (design note)

`alo-imap` is a compatibility surface, not a differentiator: JMAP is
alo's native protocol (ADR 0001), and IMAP/POP3 exist so the thirty
years of deployed clients — Thunderbird, Apple Mail, Outlook, mutt,
phones over IMAP — can reach a alo mailbox unchanged. The crate speaks
**IMAP4rev2 (RFC 9051)** and **IMAP4rev1 (RFC 3501)** — most clients
still negotiate rev1 — plus **IDLE (RFC 2177)**, **special-use (RFC
6154)**, and **MOVE (RFC 6851)**, and a minimal **POP3 (RFC 1939)**. It
is a thin protocol translator over [`AccountStore`]: every mailbox,
message, flag, and UID it serves is account-scoped data access, so the
isolation guarantee is inherited from the store, not re-implemented here.

## Surface & transport

- **IMAP:** implicit TLS on **993** and cleartext-then-**STARTTLS** on
  **143**. A submission-style posture: on 143, `LOGIN`/`AUTHENTICATE` are
  refused until STARTTLS is active (no credentials in the clear), matching
  the SMTP crate's submission gate.
- **POP3:** implicit TLS on **995** only (no cleartext 110 — POP3 auth is
  USER/PASS in the clear, so we never offer it without TLS).
- Ports/hostname/TLS cert are configured by env, mirroring `alo-smtp`
  (`ALO_IMAP_*`, `ALO_POP3_*`). Off unless an address is set.

Transport reuse — **the shared-transport question.** `alo-smtp`
already has a rustls acceptor builder (`tls.rs`) and a plain/TLS stream
enum (`stream.rs`) for its STARTTLS swap. IMAP needs the same two pieces.

- **Rejected — extract a `alo-common` (or `alo-net`) transport
  crate now.** A shared crate earns its keep once ≥3 consumers and a
  stable seam exist; today it would mean a new crate, a shared error
  type both SMTP and IMAP must map, and a version to bump in lockstep —
  premature coupling for ~100 lines. Per CLAUDE.md's judgement clause and
  the milestone brief, **duplicated TLS/stream code beats a premature
  common crate.** `alo-imap` carries its own `tls.rs` and `stream.rs`
  (its own `ImapError::Tls`, no cross-crate dependency). The moment a
  third listener wants them (e.g. ManageSieve, or a DAV port), we extract
  the crate — with the duplication as three worked examples of exactly
  what the seam must cover.

## Errors (RFC 9051 §7)

Every command gets a **tagged** completion: `OK` (success), `NO`
(failed but understood), or `BAD` (protocol/syntax error). Server-fatal
conditions send an untagged `* BYE` then close. Store errors map at the
edge and never leak internals: `NotFound` → `NO` (or an empty result for
listings), `Conflict` → `NO [ALREADYEXISTS]`/`[INUSE]` as appropriate,
`TooLarge` → `NO [LIMIT]`, an internal `Db`/`Blob` → `NO` with a generic
text (the detail stays in `tracing`, never on the wire). Response codes
(`[UIDVALIDITY n]`, `[UIDNEXT n]`, `[PERMANENTFLAGS ...]`, `[READ-ONLY]`,
`[TRYCREATE]`, `[UIDNOTSTICKY]` — never emitted, since UIDs are sticky)
are RFC-exact.

## Tenancy & account scoping

`LOGIN`/`AUTHENTICATE` verify the interim argon2 credentials via a new
`Store::verify_login(username, password) -> Option<(TenantId, UserId)>`
(the same anti-enumeration burn as `issue_token`, minus token issuance —
the identity swap stays the one seam alo-identity replaces). The
resulting `(tenant, user)` yields an [`AccountStore`] via
`Store::for_account`, and **every** subsequent command reaches data only
through that handle. There is no code path in the crate that takes a
mailbox/UID from the wire and reaches another account's rows: a foreign
name/UID resolves to nothing under the account predicate and returns the
same `NO`/empty a truly absent object would — no existence oracle, no
"500". `INBOX` (case-insensitive, RFC 9051 §5.1) maps to the account's
`role='inbox'` mailbox; all other IMAP paths map by hierarchical name.

The wrong-tenant **and** cross-account suites extend to every command:
login as A, address B's mailbox names / UIDs / sequence numbers → denied.

## UID semantics — the hard heart (RFC 9051 §2.3.1.1)

IMAP promises each message in a mailbox a **UID**: a 32-bit,
strictly-ascending, **never-reused** integer, stable across sessions,
paired with a per-mailbox **UIDVALIDITY** that the client uses to decide
whether its cached UIDs are still valid. Getting this wrong corrupts
client caches irrecoverably (a client re-downloads everything, or worse,
shows stale bodies under reused UIDs). It is the one place this crate
adds schema.

### Schema addendum (migration 0006)

- `mailboxes.uid_validity BIGINT` — assigned once at mailbox creation
  from a monotonic sequence (`mailbox_uidvalidity_seq`), never changed
  for the life of that mailbox row. Fits the RFC's 32-bit nz-number for
  the deployment's life (4.2 B mailbox creations).
- `mailboxes.uid_next BIGINT DEFAULT 1` — the next UID to assign in this
  mailbox; **monotone, only ever increments**, including across EXPUNGE.
- `mailbox_messages.uid BIGINT` — the message's UID *in that mailbox*
  (the same message in two mailboxes has two independent UIDs — UIDs are
  per-mailbox, RFC 9051 §2.3.1.1). `UNIQUE (tenant_id, mailbox_id, uid)`.

UID assignment is done under a `FOR UPDATE` lock on the mailbox row, in
the same transaction that inserts the membership, so concurrent deliveries
serialize and never collide or gap-reuse. A message is assigned its UID
exactly when it joins a mailbox (ingest, `APPEND`, `COPY`/`MOVE`,
`add_to_mailbox`); removing and re-adding a message yields a **new,
higher** UID (correct: to the client it is a new message in that mailbox).

### When UIDVALIDITY changes — never silently

- For a live mailbox: **never.** UIDs are sticky; `uid_next` only grows;
  UIDVALIDITY is constant. Clients keep their caches across reconnects.
- A mailbox `DELETE`d and a new one `CREATE`d with the same name is a
  **new row** with a fresh `uid_validity` from the sequence. To the
  name-addressing client this reads as a UIDVALIDITY change and it
  correctly discards its cache — which is exactly right, because the UIDs
  it cached belonged to a mailbox that no longer exists. We never reuse a
  `uid_validity` value, so a client can always trust that an unchanged
  UIDVALIDITY means unchanged UID meaning. We never emit `[UIDNOTSTICKY]`.

### Sequence numbers vs UIDs

The message **sequence number** is the 1-based position of a message in
the mailbox ordered by UID ascending (RFC 9051 §2.3.1.2). The session
holds an ordered snapshot (a `Vec` of UIDs) of the selected mailbox — the
"view". Sequence-number commands index the view; `UID` commands address
UIDs directly. `EXPUNGE` removes from the view and renumbers **sequence
numbers** (emitting `* n EXPUNGE` in ascending order, each number
decrement-adjusted for the removals already reported), but never touches
UIDs. New arrivals append and bump `EXISTS`. This is the whole reason a
server keeps per-session state: the view is refreshed on `SELECT` and
mutated in lockstep with the untagged responses the client is told about.

### Rejected UID mappings

- **Reuse the store `MessageId` (opaque base64url) as the UID.** Rejected:
  UIDs must be numeric, 32-bit, per-mailbox, and *ascending in arrival
  order*; an opaque random id is none of these. It also conflates the
  global message identity with its per-mailbox position.
- **A single global per-message integer as the UID everywhere.** Rejected:
  UIDs are per-mailbox (the same message COPYed into two mailboxes needs
  two UIDs), and a global counter gives non-contiguous UIDs per mailbox
  and no per-mailbox UIDVALIDITY epoch. The per-mailbox `uid_next` is the
  only shape that satisfies §2.3.1.1.
- **Derive the UID from `added_at` (timestamp).** Rejected: collisions at
  equal timestamps, not contiguous, and re-clocking risks non-monotonic
  UIDs — a cache-corruption bug waiting to happen.

## Message retrieval fidelity (FETCH)

Byte-exact where it must be, honest where the parser is bounded:

- `FLAGS`, `INTERNALDATE` (= `received_at`), `RFC822.SIZE` (= stored
  `size`), `UID` — direct from the store.
- `ENVELOPE` — parsed on demand from the raw header block with a real
  RFC 5322 address-list parser (display-name, `<local@domain>`, comma
  lists, minimal group handling); fields we cannot parse are `NIL`, never
  guessed.
- `BODY[]`, `BODY[HEADER]`, `BODY[TEXT]`, `BODY[HEADER.FIELDS (...)]`,
  and partial `<offset.count>` — **byte-exact slices of the stored raw
  message** (clients may hash these; we never re-render). The
  header/body split is the first empty line in the raw octets.
- `BODYSTRUCTURE`/`BODY` and `BODY[<part>]` — a **bounded** recursive
  MIME walk (depth ≤ 16, ≤ 256 parts): single-part and `multipart/*`
  trees decompose correctly (type/subtype, params, encoding, octet size,
  line count for text parts); extension fields we do not compute
  (MD5, disposition, language, location) are `NIL`. A message whose MIME
  is malformed past the bound is served as a single `text/plain` part —
  honest degradation, never a fabricated tree. The limit is stated in
  `CAPABILITY` docs and `docs/interop.md`, not hidden.

`\Recent` is always reported as 0 / `PERMANENTFLAGS` omits it: RFC 9051
retires `\Recent`, and rev1 permits a server that never sets it. We do
not track it (it needs exclusive-session semantics we deliberately skip).

## Commands out of scope (recorded)

CONDSTORE/QRESYNC (`MODSEQ`) — the store has a per-account modseq but no
per-message mod-sequence, so we do **not** advertise them; SORT/THREAD
extensions; ACL/QUOTA/METADATA/NOTIFY/COMPRESS/BINARY/CATENATE;
MULTIAPPEND; server-side `SEARCH` charsets beyond US-ASCII/UTF-8; POP3
APOP and PIPELINING. `\Recent`, as above. These are additive later and
named here so their absence is a decision, not an oversight.

## POP3 (the approved cut seam)

`alo-pop3` (module, same crate): implicit TLS on 995, `USER`/`PASS`
→ `verify_login`, then `STAT`/`LIST`/`RETR`/`DELE`/`RSET`/`NOOP`/`QUIT`/
`UIDL`/`TOP`, **inbox only**. `UIDL` reuses the inbox UIDs (stable), so a
POP3 client's "leave on server" bookkeeping is consistent with IMAP.
`DELE` marks for deletion; `QUIT` in TRANSACTION commits (removes from
inbox). If the session runs short, POP3 is the sanctioned cut — the IMAP
scope (transport through IDLE) is not cuttable.

## Verification

Quality gate incl. the extended isolation targets; a raw `openssl
s_client` transcript of `LOGIN → LIST → SELECT → FETCH → STORE → IDLE`
receiving a live untagged update as SMTP delivers into the selected
mailbox; and a real-client pass (Thunderbird / `imaplib`) with every
forced quirk recorded in `docs/interop.md`.
