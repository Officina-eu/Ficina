# Interop log

Client quirks and RFC deviations. Format per entry: date · client+version · quirk observed · our response · RFC section affected.

(no client-forced entries yet — and every entry added here is a debugging session nobody repeats)

## Standing policies (deliberate strictness/tolerance choices, not client-forced)

- 2026-07-25 · **Bare LF / stray CR rejected everywhere** · RFC 5321 §2.3.8 requires
  CRLF; bare line endings are the SMTP-smuggling vector. Commands: 500, session
  continues. Inside DATA: 500 and the connection **closes** (the stream cannot be
  trusted to re-sync). Matches Postfix `smtpd_forbid_bare_newline=yes` posture.
- 2026-07-25 · **One space tolerated after `MAIL FROM:`/`RCPT TO:`** · RFC 5321
  §4.1.1.2 admits no space; many clients send one; no security ambiguity → accepted.
- 2026-07-25 · **DATA line length** · §4.5.3.1.6 sets 1000 octets as the sender
  limit; long HTML lines are routine in real mail. We accept content lines up to
  8192 octets and reject the message (500, drained, session survives) beyond —
  tolerance with a defensive ceiling.
- 2026-07-25 · **8-bit bytes accepted in DATA without 8BITMIME advertised** ·
  Strictly, unadvertised 8BITMIME means 7-bit only (RFC 6152); rejecting 8-bit
  bodies would bounce half of real-world mail. Accepted, like every mainstream MTA.
  8BITMIME advertisement lands with the capability milestone (M3).
- 2026-07-25 · **General address literals rejected 501** · §4.1.3 tagged literals
  (`[tag:content]`) are syntactically legal; nothing routable can be done with
  them; IPv4 and `IPv6:` literals are accepted.
- 2026-07-25 · **Source routes accepted and ignored** · §4.1.2 / Appendix C:
  `<@relay:user@dom>` parses, the route is validated then discarded.
- 2026-07-26 · **DATA has a 10-minute total budget** · §4.5.3.2 specifies
  per-wait timeouts; we additionally bound the whole message receive at 600 s as
  anti-flood policy. A legitimate sender slower than ~350 kbps on a 25 MiB
  message would be cut off (421); accepted trade-off, revisit with real traffic.
- 2026-07-26 · **EHLO/HELO argument restricted to printable ASCII** · §4.1.1.1
  expects a Domain/address-literal; we reject control octets with 501 so the
  attacker-controlled greeting can never inject binary into the Received: stamp
  or the spool sidecar. SMTPUTF8 (M3) will widen this to U-labels.
- 2026-07-27 · **Outbound delivery off by default** · M1 accepts any recipient;
  turning on relaying before the AUTH gate (M3) would make an exposed instance
  an open relay. Delivery requires `FICINA_SMTP_OUTBOUND_ENABLED=true`, and a
  smarthost route is the supported self-hosted mode until MX+AUTH are complete.
- 2026-07-27 · **Empty MAIL FROM on outbound = null path** · we send
  `MAIL FROM:<>` for DSNs and never generate a DSN for a message that itself
  arrived with a null reverse-path (RFC 5321 §4.5.5 loop prevention).
- 2026-07-27 · **Domainless recipients parked, not delivered** · a bare
  `<postmaster>` (§4.1.1.3) has no domain to route to; M2 holds such messages
  in the spool (logged) pending local delivery (M5) rather than dropping or
  bouncing them.
- 2026-07-27 · **STARTTLS discards all pre-TLS state** · RFC 3207 §4.2: after a
  successful STARTTLS the HELO/EHLO identity, any transaction, and any prior
  auth are cleared; the client must EHLO again. Buffered plaintext arriving
  after our 220 and before the handshake is treated as a command-injection
  attempt (CVE-2011-0411 class) and the connection is dropped, nothing executed.
- 2026-07-27 · **AUTH offered only on submission over TLS** · `AUTH PLAIN`/`LOGIN`
  are advertised and accepted only on a submission listener with TLS active.
  On the MX (port 25) role AUTH is refused 503; before TLS it is refused 538.
  Wrong password and unknown user return the same 535 (anti-enumeration, §7.3).
- 2026-07-27 · **EHLO capabilities are state-exact** · we advertise STARTTLS only
  while TLS is inactive, AUTH only on submission-over-TLS, and always SIZE and
  8BITMIME. Advertising a capability implies accepting its MAIL parameters, so
  `SIZE=`, `BODY=7BIT|8BITMIME`, and `AUTH=` (RFC 4954 §5, accepted and ignored)
  are honored; every other MAIL parameter is still 555.
- 2026-07-27 · **Submission adds Date/Message-ID only** · RFC 6409 §8 permits the
  MSA to rewrite more, but we make the minimal non-destructive fix (add `Date:`
  and `Message-ID:` when absent) and never touch `From`/`Sender` or the body.
- 2026-07-27 · **Submission requires STARTTLS then AUTH** · on submission
  ports (587/465) MAIL before TLS gets 530 (must STARTTLS) and MAIL before a
  successful AUTH gets 530 (auth required) — the open-relay gate. MX (25)
  authenticates no one and never advertises AUTH.
- 2026-07-27 · **Authentication-Results is the verdict contract** · every
  SPF/DKIM/DMARC (and later ARC/spam) result is recorded in one
  `Authentication-Results` header (RFC 8601) under one authserv-id (our
  hostname). Downstream (store/JMAP/UI) parses THIS, not internal types; the
  rendered format changes additively only. `Received-SPF` is also stamped for
  operators/legacy tooling but is not the authoritative record.
- 2026-07-27 · **Malformed auth input fails, never crashes** · a malformed
  SPF/DKIM/DMARC record, DKIM signature, or DNS key (all internet-sourced)
  yields a fail/permerror verdict, never a panic — enforced by the workspace
  unwrap/panic deny-lints plus fuzz-style tests and a bounded hand-rolled DER
  parser for DKIM public keys.
- 2026-07-27 · **DMARC disposition** · `p=reject` + authenticated-fail → 550 at
  DATA. `p=quarantine` is accepted (the verdict is recorded in
  Authentication-Results; actual foldering is a store concern, M5). SPF `ptr`
  is implemented but discouraged (RFC 7208 §5.5).
- 2026-07-27 · **RSA crypto via ring, not the rsa crate** · the `rsa` crate
  carries the unfixed Marvin timing sidechannel (RUSTSEC-2023-0071); DKIM RSA
  sign/verify use ring (constant-time). DKIM public keys (SPKI) are unwrapped
  to PKCS#1 by a small bounded DER parser before ring verification.
- 2026-07-27 · **Rspamd fail-closed at DATA (M4b)** · when a scanner is
  configured (`FICINA_SMTP_RSPAMD_URL`) and is unreachable / times out / answers
  unparseably, the message is deferred **451**, not accepted — a scanner outage
  must never silently disable filtering. `reject` → 550, `soft reject`/`greylist`
  → 451, else accept with an `x-spam` method in Authentication-Results. DMARC
  `p=reject` is evaluated *before* the spam verdict, so an authenticated-fail is
  a 550 DMARC rejection regardless of spam score. Verified end-to-end against
  real Rspamd 4.1.2 (GTUBE → 550).
- 2026-07-27 · **Rspamd request metadata is CR/LF-stripped** · the envelope
  fields we pass to `/checkv2` (`IP`/`Helo`/`From`/`Rcpt`/`MTA-Name`) are
  attacker-controlled; control characters are stripped so a crafted MAIL FROM
  cannot inject extra HTTP headers into the scanner request.
- 2026-07-27 · **MTA-STS served in plaintext behind the TLS proxy (M4b)** ·
  RFC 8461 §3.2 mandates HTTPS with a WebPKI-valid cert on `mta-sts.<domain>`;
  `ficina-smtp` serves the policy over plaintext HTTP on
  `FICINA_SMTP_MTA_STS_ADDR` and the deploy reverse proxy terminates TLS. The
  policy `id` is derived from the policy content, so it rotates automatically on
  any change. **DNS records to publish:** `_mta-sts.<domain> TXT "v=STSv1;
  id=<the id we render>"` and `mta-sts.<domain>` A/AAAA (or CNAME) pointing at
  the proxy that fronts the policy endpoint. TLS-RPT (`_smtp._tls`) reporting is
  deferred.
- 2026-07-27 · **Inbound trust headers stripped before stamping** · RFC 8601
  §5: on the MX boundary we delete any pre-existing `Authentication-Results`
  bearing our own authserv-id, and any `Received-SPF`, before adding ours — a
  remote sender must not be able to plant the verdict header downstream trusts.
  A different authserv-id's `Authentication-Results` (a legitimate upstream) is
  preserved.
- 2026-07-27 · **DKIM `From` must be signed** · RFC 6376 §6.1.1: a signature
  whose `h=` omits `From` is a permerror, not a pass — otherwise the visible
  sender could be altered while DKIM still reported pass.
- 2026-07-27 · **DKIM `l=` counts canonicalized octets** · §3.7: inbound `l=`
  is applied after body canonicalization, not before, so `simple`-body
  signatures with trailing-whitespace differences score correctly. Our signer
  omits `l=` by default (it permits post-signing appends) but can emit it.
- 2026-07-27 · **DMARC `pct` sampled with a non-crypto draw** · §6.6.4: for the
  `100 - pct` fraction "sampled out", the next-lower policy applies
  (reject→quarantine→none). The per-message draw is a sub-nanosecond timestamp
  sample — sufficient for policy sampling, not a security decision.
- 2026-07-27 · **SPF `redirect=` to a recordless domain is permerror** · §6.1:
  a redirect whose target publishes no (or a malformed) SPF record is a
  permerror, distinct from a bare `none`. A no-record lookup also charges the
  §4.6.4 void-lookup budget.
- 2026-07-27 · **Non-UTF-8 header octet drops only its own field** · a stray
  8-bit byte in one header no longer erases the whole header block (which would
  silently void DKIM/DMARC for the message); each header's UTF-8 is validated
  in isolation. A multi-address `From` with differing domains yields no DMARC
  From-domain (RFC 7489 §6.6.1).
- 2026-07-27 · **JMAP mailbox thread counts approximate email counts** ·
  RFC 8621 §2 requires `totalThreads`/`unreadThreads`; the interim store does
  not compute distinct thread counts per mailbox, so `ficina-jmap` reports them
  equal to `totalEmails`/`unreadEmails`. Clients rely mainly on
  `unreadEmails`; exact thread counts are an additive refinement.
- 2026-07-27 · **JMAP state tokens are one per-account modseq shared by types** ·
  Mailbox/Email/Thread states are the same opaque per-account counter (keyed
  by `(tenant, user)`, migration 0005), so a type's state may skip numbers
  when another type changed. State tokens are opaque (clients must not parse
  them), so this is spec-conformant and lets `/changes` for any type resume
  correctly. The counter is per-account, not per-tenant, so a co-tenant user's
  activity never advances another's cursor — closing the activity-volume side
  channel that a shared tenant modseq left open (the invariant IMAP IDLE
  relies on for account-scoped push).
- 2026-07-27 · **JMAP isolation is per-account (accountId = user)** · every
  by-id read/mutate, `/changes`, `Thread/get`, and blob download is scoped to
  the token's `(tenant, user)` — a user cannot reach another user's mail even
  within the same tenant. Threads are resolved per-account (no cross-user merge
  on a shared Message-ID). Enforced by `owns_*` guards + a `user_id`-scoped
  change log; covered by a two-users-in-one-tenant isolation test.
- 2026-07-27 · **JMAP `Email/set` per-object update is not yet atomic** ·
  RFC 8620 §5.3 wants a single object's changes applied all-or-nothing; the
  interim maps each keyword/mailbox change to its own store transaction, so a
  mid-update failure can leave a partial (but individually consistent) result
  reported under `notUpdated`. A single-transaction multi-change store method is
  the follow-up. Mixed full-`keywords` + `keywords/X` patches are applied
  sequentially rather than rejected as `invalidPatch` (documented, not fatal).
- 2026-07-27 · **JMAP rate-limiting / concurrent-upload caps deferred to the
  gateway** · `/auth/token` throttling and `maxConcurrentUpload` enforcement
  belong at the gateway/identity layer (ficina-identity); `maxObjectsInGet`,
  `maxSizeRequestObject` (per-route body limit), and pagination caps ARE
  enforced in-process now.
- 2026-07-27 · **JMAP auth is interim bearer tokens (not OIDC yet)** ·
  `/auth/token` issues opaque bearer tokens against argon2 credentials in the
  store; the token → `(tenant, account)` resolution is the seam ficina-identity
  (OIDC) replaces later. No cookies, no OAuth flow yet (deliberate — see
  `docs/design/jmap-api.md`).
- 2026-07-27 · **Store threading is references-only and forward-only** ·
  `ficina-store` threads a message onto an *earlier* message it references
  (`In-Reply-To`/`References`); it does not merge on base subject alone
  (RFC 8621 §3 permits subject as a tiebreaker, but subject-only merging bleeds
  unrelated conversations, so we omit it). Consequence: a reply delivered
  before its parent, or a `Re:` with no `References`, starts its own thread and
  is not retro-merged. Accepted trade-off; revisit if real mail shows
  meaningful out-of-order fragmentation.

## IMAP / POP3 shims (ficina-imap)

- 2026-07-27 · **Real-client interop: Python imaplib 3.14 + raw openssl s_client** ·
  the IMAP shim was driven end-to-end by Python's `imaplib` (a real, widely-
  deployed client library) over implicit TLS: LOGIN, LIST, SELECT, FETCH
  (ENVELOPE + `BODY[HEADER.FIELDS]`), STORE ±FLAGS, SEARCH, UID SEARCH, and
  APPEND (returning `[APPENDUID]`) all succeeded with **no accommodation
  required**, and a raw `openssl s_client` transcript confirmed the full loop
  including IDLE receiving an untagged `* n EXISTS` as a message was delivered
  into the selected mailbox. **Thunderbird's GUI could not be driven in this
  headless environment**; `imaplib` + the openssl transcript stand in as the
  real-client evidence for this milestone, and a Thunderbird desktop pass is a
  recorded follow-up for when a GUI environment is available. No client-forced
  quirk surfaced; the entries below are our deliberate strictness/model choices.
- 2026-07-27 · **INBOX is auto-provisioned at LOGIN** · RFC 9051 §5.1 makes
  INBOX always present; the store creates a user's inbox lazily, so the IMAP
  session calls `AccountStore::inbox()` right after authentication so LIST/
  SELECT/APPEND never see a missing INBOX on a brand-new account.
- 2026-07-27 · **HEADER.FIELDS returns message order, byte-exact** · RFC 9051
  §7.5.2: `BODY[HEADER.FIELDS (…)]` returns the named fields **in the order they
  appear in the message**, not the requested order, as exact byte slices of the
  stored header (clients may hash them). `BODY[]`/`[HEADER]`/`[TEXT]` are exact
  slices too.
- 2026-07-27 · **Auth only over TLS; POP3 TLS-only** · IMAP `LOGIN`/
  `AUTHENTICATE` are refused before STARTTLS with `NO [PRIVACYREQUIRED]` and
  `LOGINDISABLED` is advertised pre-TLS (RFC 9051 §7.1.1); POP3 is served only
  on implicit TLS (995), never cleartext 110, since USER/PASS are in the clear.
  Both cap failed authentications per connection and drop the socket.
- 2026-07-27 · **Flags are shared across mailboxes (JMAP model)** · a message
  COPYed into two mailboxes is one store object with one keyword set, so setting
  `\Seen` in one mailbox is visible in the other — a deliberate divergence from
  classic IMAP copy-independence, inherited from the JMAP-native store. Most
  clients do not rely on per-copy flag independence.
- 2026-07-27 · **`\Recent` not tracked; `\Deleted` is the `$deleted` keyword** ·
  RFC 9051 retires `\Recent`, so we always report `0 RECENT` and omit it from
  PERMANENTFLAGS. `\Deleted` has no JMAP standard keyword and is stored as the
  internal `$deleted` keyword; EXPUNGE removes `$deleted` messages from the
  mailbox (destroying a message only when it is left in no mailbox).
- 2026-07-27 · **Hierarchy separator is `/`** · IMAP mailbox paths join name
  segments with `/`; a stored mailbox name that itself contains `/` is
  ambiguous over IMAP and is a documented shim limit (JMAP-created names
  normally do not). Mailbox names with control characters are rejected at
  CREATE/RENAME (a CR/LF in a name would otherwise splice response lines).
- 2026-07-27 · **BODYSTRUCTURE fidelity is bounded and honest** · single-part
  and `multipart/*` trees decompose correctly; MIME malformed past depth 16 /
  256 parts degrades to a single `text/plain` part rather than emitting a
  fabricated structure. Extension fields we do not compute (MD5, disposition,
  language) are `NIL`. CONDSTORE/QRESYNC are **not** advertised (no per-message
  mod-sequence). See `docs/design/imap-pop3-shims.md`.
- 2026-07-27 · **IDLE is poll-driven off the per-account change cursor** · RFC
  2177 push is delivered by watching this account's own modseq (migration 0005)
  at a 1 s cadence and diffing the selected-mailbox view; sub-second LISTEN/
  NOTIFY is a follow-up. The cursor is per-account, so an IDLE stream is
  provably silent about another account's activity.
- 2026-07-27 · **MOVE into the source mailbox is a no-op** · RFC 6851 MOVE of a
  message into the mailbox it already occupies must not lose it; we detect
  same-mailbox MOVE and leave the message untouched (an earlier draft would have
  expunged the sole membership — caught in review, now regression-tested).

## Inbound local delivery (SMTP → store)

- 2026-07-28 · **Unknown local user is refused `550 5.1.1` at RCPT** · when
  local delivery is configured (MX + `DATABASE_URL` + hosted domains), each
  hosted-domain `RCPT TO:` is resolved against the store; an unknown mailbox
  gets `550 5.1.1 No such user here` at RCPT, not a post-DATA drop or bounce.
  This is a deliberate recipient-enumeration oracle (a prober learns which
  local addresses exist) — the conscious, mainstream choice: silent-accept-
  then-drop loses mail the sender was told 250 for, and post-DATA bounces are
  backscatter. Enumeration is mitigated at the edge (rate limits), not by
  lying to senders. A non-local recipient is still `550 5.7.1 Relaying denied`
  first (the two policies compose). See `docs/design/local-delivery.md`.
- 2026-07-28 · **Every accepted recipient — including `<postmaster>` — is
  resolved at RCPT** · so no recipient accepted at RCPT can turn out
  unresolvable at DATA and defer the whole message (which would let a hostile
  sender append a never-resolvable recipient to force repeated duplicate
  delivery to the valid ones). `<postmaster>` (RFC 5321 §4.1.1.3) resolves as
  `postmaster@` the first hosted domain and therefore needs a backing mailbox
  to receive.
- 2026-07-28 · **Multi-recipient partial failure → one 4xx (dup over loss)** ·
  DATA carries a single reply (RFC 5321 §4.1.1.4). A transient store fault for
  any recipient returns `451 4.3.0` for the whole message so the sender
  retries — a redelivery to an already-committed recipient (a duplicate,
  deduped to one blob by content-addressing) is strictly safer than losing
  mail (RFC 5321 §6.1). Per-recipient DSN is additive later.
- 2026-07-28 · **Local delivery uses a durable filesystem blob backend** ·
  message bytes are written to `FICINA_SMTP_BLOB_DIR` (default `./blobs`) via
  the store's on-disk backend, so a delivered body survives a restart (the DB
  row is only the commit point after the blob is durable). Multi-node
  production swaps in Garage/S3 behind the store's `garage` feature.
- 2026-07-28 · **Inbound local mail bypasses the spool; the spool stays the
  outbound queue** · received mail for a hosted domain lands in the store, not
  the relay spool. Any pre-existing all-local spool entries are migrated into
  the store once at startup (before the outbound queue runner starts); a
  crash between deliver and spool-removal re-delivers a deduped duplicate on
  the next start, never a loss.

## Sieve filtering (ficina-sieve)

- 2026-07-27 · **Sieve runs at the store delivery entry; SMTP local delivery
  is M5** · the engine filters at `AccountStore::deliver_sieve` — the ingestion
  boundary, after spam scoring, before filing. SMTP still *spools* inbound mail
  rather than delivering it into the store (local delivery is M5, a separate
  ROADMAP item), so the swaks → SMTP → mailbox wire is not yet closed; the seam
  is ready (`Store::account_by_email` + `deliver_sieve`). Until M5, the Sieve
  path is exercised through that entry (migration tool, tests), and the
  redirect/vacation `OutboundAction`s are returned to the caller for the future
  bridge to enqueue.
- 2026-07-27 · **Rule management is JMAP for Sieve, not ManageSieve** · ADR 0007:
  `SieveScript/{get,set,validate}` over the same bearer-auth account door as
  Mail, compile-checked on `set` (`invalidScript`). ManageSieve (RFC 5804) is a
  deliberate rejection (additive later). One documented deviation from RFC 9661:
  script `content` is carried **inline** rather than via a `blobId` round-trip
  (blob-based content is additive).
- 2026-07-27 · **fileinto auto-create is OFF; a missing folder keeps to Inbox** ·
  RFC 5228: `fileinto "Nope"` to a non-existent mailbox is not created — it
  degrades to implicit keep into the Inbox with a logged warning, so a typo
  never spawns folders and mail is never lost.
- 2026-07-27 · **No script failure loses mail** · RFC 5228 §2.10.6: a compile
  error on the active script, an evaluation budget overrun, or an unperformable
  action all fall back to implicit keep into the Inbox — never a bounce, never a
  drop.
- 2026-07-27 · **imap4flags map to JMAP keywords** · Sieve/IMAP flags are mapped
  to the store's canonical keywords (`\Seen` → `$seen`, RFC 8621 §4.1.1) before
  filing; `\Recent` and unknown system flags are unsettable and dropped rather
  than persisted as a bogus `\`-keyword.
- 2026-07-27 · **Redirect and vacation are bounded at the source** · a runaway
  or hostile script cannot storm: per-script redirect cap (3), per-account
  redirect rate budget, loop guards (Auto-Submitted / null return-path /
  Received-count ceiling), self-redirect refusal; vacation carries the full
  RFC 3834 guard set and per-correspondent `:days` suppression. A per-account
  vacation-send budget and alias-aware self checks are recorded follow-ups (the
  vacation path is 1:1 with no amplification and its outbound is M5-deferred).
