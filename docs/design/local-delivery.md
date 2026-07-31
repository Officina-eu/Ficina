# Inbound local delivery (design note)

This closes the spine both prior sessions left open: inbound SMTP has
terminated at a spool, so a received message never reached `alo-store`,
which is why Sieve and IMAP could not show end-to-end evidence and why
dogfooding was blocked. This milestone carries a received message the last
hop — SMTP → store — with Sieve at the boundary. It is **wiring, not new
protocol**: the parts already exist (`Store::account_by_email`,
`AccountStore::deliver_sieve` returning store + outbound actions, the
store's crash-safe ingestion, the outbound queue). The only risk is
threading a store handle through the SMTP runtime without leaving a
half-built change; the arc is worked whole.

## Shape

`alo-smtp` gains an optional `LocalDelivery` (a `Store` handle + the
inbound `Spool` for enqueuing Sieve's outbound actions). It is wired only
on the **MX** role and only when a database URL is configured; the
submission role and the outbound queue are untouched.

- **RCPT time — recipient resolution.** On the MX role with local
  delivery configured, each `RCPT TO:` for a hosted domain is resolved via
  `Store::account_by_email`. An **unknown local user is refused `550 5.1.1`
  at RCPT** (not after DATA). The existing anti-open-relay guard still
  refuses non-local recipients to an unauthenticated sender first, so the
  two policies compose: non-local → relay-denied; local-but-unknown →
  no-such-user; local-and-known → accepted.
- **DATA time — delivery.** After the trust stack and spam scoring have
  stamped their headers (unchanged), the fully-built message (our
  `Received:` + `Authentication-Results` + body) is delivered to **each**
  resolved recipient through `AccountStore::deliver_sieve`: parse → run the
  account's active Sieve script → file into that account's own mailboxes.
  Isolation is inherited from `account_by_email → for_account`; one
  recipient's script runs on and files into only that recipient's mailbox.
  Local mail no longer touches the spool.
- **Sieve outbound actions.** `redirect`/`vacation` returned by
  `deliver_sieve` are enqueued through the **existing** outbound queue (the
  inbound `Spool` the queue drains), with the same redirect-rate/loop
  budget the store already applies — now exercised on the real path.
  Attacker-influenced strings (`subject`/`from`/redirect `address`) are
  **CR/LF-stripped before any header is built** (the injection guard the
  Sieve session's audit flagged as an M5 must-do).

## Durability

Delivered message bytes go to a **durable on-disk blob backend**
(`BlobStore::local`, object-store's filesystem backend, rooted at
`ALO_SMTP_BLOB_DIR`, default `./blobs`), so a delivered body survives a
restart — the store's commit point (the DB row) is only reached after the
blob is durably written. Multi-node production swaps in Garage/S3 behind
the store's `garage` feature; the in-memory backend is tests only. A single
message with up to `max_rcpt` (100) local recipients does up to 100
synchronous `deliver_sieve` calls on one connection's task — bounded by the
recipient and message-size caps, and confined to that task (it cannot
starve other connections); a per-message parallelism/work bound is a
recorded follow-up if volume warrants.

## Failure semantics — no mail loss on the live path

Delivery is per-recipient, try-then-commit:

- **Unknown local user** — permanent — is the `550 5.1.1` at RCPT (above),
  so it never reaches DATA. **Every** accepted recipient (including
  `<postmaster>`, resolved as `postmaster@` the first hosted domain) is
  resolved at RCPT, so no accepted recipient can turn out unresolvable at
  DATA and defer the whole message — which would let a hostile sender append
  a never-resolvable recipient to force repeated duplicate delivery to the
  valid ones. `<postmaster>` therefore needs a backing mailbox to receive.
- **A transient store/blob failure** for any recipient at DATA yields a
  **4xx** at the end of DATA so the sender retries the whole message.
- **Crash between blob write and mailbox commit** resolves per the store's
  documented crash-safety (`docs/design/message-store.md`): the blob is
  written first and the DB commit is the commit point, so a crash leaves an
  invisible orphan blob (GC'd) and an *unaccepted* message — the SMTP 4xx
  makes the sender resend. **The blob-write side never wins as a visible
  half-message; nothing is lost.**

## Rejected alternative — recipient enumeration: bounce vs. silent-accept

Refusing an unknown local user at RCPT (`550 5.1.1`) tells a prober which
local addresses exist — a recipient-enumeration oracle. The alternative,
**accept every local-domain recipient at RCPT and silently drop (or
bounce) unknowns after DATA**, hides valid addresses but is worse on every
axis that matters more: silent-drop *loses mail a sender was told 250 for*
(violating our first law), and post-DATA bounces are backscatter to forged
senders (the exact spam-reflector we bound elsewhere). **We choose the
`550` at RCPT deliberately** — it is what every mainstream MTA does, it
gives legitimate senders an immediate honest answer, and it never loses or
backscatters mail. The enumeration exposure is real and accepted; it is
mitigated at the edge (connection rate limits, fail2ban-style controls at
the gateway), not by lying to senders. Recorded in `docs/interop.md`.

## Rejected alternative — multi-recipient partial failure: per-recipient DSN vs. conservative 4xx

A single `DATA` has **one** reply for all recipients (RFC 5321 §4.1.1.4);
per-recipient outcomes are expressible only at RCPT (already done for
unknown users) or afterward via DSNs. When some recipients commit and one
hits a transient store error, the options are (a) `250` and generate a DSN
bounce for the failed recipient, or (b) `4xx` for the whole message so the
sender retries all. **We choose (b), the conservative reply**, because RFC
5321 §6.1 is explicit that *duplicate delivery is preferable to loss*: a
`4xx` may redeliver to the already-committed recipients (a duplicate — the
store dedups blobs by content, so only a second mailbox row, not a second
copy of bytes), which is strictly safer than a DSN path that can lose or
backscatter. Per-recipient DSN handling is additive later once the DSN
generator (M2) is trusted on the local path. Each recipient's Sieve runs
independently and a Sieve failure is never an error (implicit keep), so
"one recipient's rule failure must not affect another's" holds — only a
genuine transient store fault escalates to the whole-message 4xx.

## Retiring the spool as the inbound sink

Inbound local mail lands in the store, not the spool. Any messages already
sitting in the spool that are destined **entirely** for local recipients
are migrated into the store **once at startup**, before the outbound queue
runner starts (so there is no concurrent claim), then removed from the
spool; entries with a non-local recipient stay for the outbound queue. The
spool remains the **outbound** queue's durable store (M2, unchanged).

## Out of scope (recorded)

Per-recipient DSN at DATA (above); `<postmaster>` without a backing account
(accepted at RCPT, delivered if an account exists, else the conservative
4xx — a dedicated alias table is alo-identity's job); catch-all / alias
expansion (alo-identity); and the general spool-migration tool for a
mixed inbound/outbound backlog beyond the all-local startup pass.
