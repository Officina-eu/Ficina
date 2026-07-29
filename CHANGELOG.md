# Changelog

User- and operator-visible changes, written when the knowledge is
fresh (release skill). Versions follow SemVer against public
contracts.

## Unreleased

- New: **Sending mail** — JMAP **`EmailSubmission/set`** (RFC 8621 §7), so the
  web app's Compose and Reply actually send. A composed message is built as a
  proper RFC 5322 `text/plain` message (all To/Cc, reply threading, and
  European-correct non-ASCII via RFC 2047 encoded-words + base64 body — no
  header injection) and sent through a new **trusted internal SMTP submission
  listener** so it is DKIM-signed, queued, and delivered by the existing
  outbound path, then filed to Sent. **Send-as is enforced on both the SMTP
  envelope and the visible `From:` header** (a token cannot send as another
  identity), only drafts are sendable, and recipients are capped per message.
  The outbound SMTP client is now a shared `ficina-smtp-client` crate used by
  both the delivery path and this submission path (no duplication). New config:
  `FICINA_SMTP_INTERNAL_SUBMISSION_ADDR` (never publish this port) and
  `FICINA_JMAP_SUBMISSION_ADDR`. Design + security review:
  `docs/design/email-submission.md`.
- New: **Ficina web app** — the one-product workspace shell, web-first
  (`web/`). The "warm workshop" design system (paper / verdigris / copper /
  ink tokens, self-hosted Inter + EB Garamond, shared primitives), the left
  rail + layout frame with a module registry that Agenda/Chat/Drive/Docs plug
  into later, first-party **OIDC + PKCE** sign-in against `ficina-identity`
  (2FA field revealed on demand), and a **Mail read surface** — folders,
  message list, and a reading pane that renders plain text in Garamond and
  isolates untrusted HTML in a sandboxed, CSP-locked iframe that blocks remote
  content (no tracking pixels). Served at the same origin as the API behind
  Caddy; sign-in verified end-to-end on the live deployment. Compose/reply,
  PWA/offline, and the other modules are the next items. Design note
  `docs/design/web-shell.md`.
- New: **`ficina-identity`** — the credential authority and an **OpenID
  Connect / OAuth 2.0 provider** (Ficina-as-IdP). It replaces every interim
  auth path: SMTP AUTH, IMAP/POP3 `LOGIN`, and the JMAP bearer now
  authenticate through one crate, and the dev `StaticAuthenticator`, the
  store's interim `auth.rs`, and the SMTP credentials-file loader are
  **deleted**. Passwords are **argon2id** (OWASP-baseline parameters,
  documented as a contract and overridable per deployment); **every secret
  comparison is constant-time** (the `subtle` crate), and an unknown user
  still pays one argon2 hash so *wrong password* and *no such user* are
  indistinguishable in time — closing the timing oracle the M3 TLS audit
  pinned here (proven by a timing test, not asserted: unknown-vs-wrong
  ratio ≈ 1.0). Tokens and recovery codes are stored only as SHA-256
  hashes; secrets never appear in a log, error, or `Debug`. The identity
  model is **tenants → users → aliases + groups**; `account_by_email`
  (inbound routing) is **alias-aware**; a tenant's first admin is created
  by the `identityctl` **CLI**, never a public endpoint. The **OAuth
  provider** offers discovery (RFC 8414), a JWKS, `authorization_code` with
  **mandatory PKCE `S256`** (RFC 6749/7636 — `plain` and challenge-less
  codes refused), and token / userinfo / revocation (RFC 7009). **Access
  tokens are opaque and revocable** (a logout truly invalidates); refresh
  tokens rotate on use and a replayed refresh token **revokes the whole
  token chain**; authorization codes are single-use. **ID tokens are EdDSA
  (Ed25519) JWTs** with `kid` rotation designed in — `sub` is the stable
  opaque user id, never the email (ADR 0008 explains opaque-vs-JWT and
  EdDSA-vs-RS256). **TOTP 2FA** (RFC 6238) adds enrollment (provisioning
  URI), verification with a clock-drift window, and single-use recovery
  codes. **2FA is enforced everywhere it can be:** the OIDC flow prompts for
  the code, and the legacy protocols (IMAP/POP3/SMTP), which cannot prompt,
  **fail closed** for a TOTP-enabled account — a password-only login is
  refused (indistinguishably from a wrong password) so a phished password
  cannot bypass 2FA over IMAP. Credential endpoints — including the legacy
  ones — have per-`(client, )username` exponential backoff (not a lockout,
  which would be a denial-of-service lever). Reviewed + security-audited
  (two independent passes); cross-tenant **and** cross-account isolation is
  tested on every identity operation, and the OAuth flow's negative cases
  (wrong PKCE verifier, code/refresh replay → chain revoke, unregistered
  redirect, bad credentials) are covered. App-specific passwords + `XOAUTH2`
  are the sanctioned follow-up that lets a 2FA user drive a non-OAuth legacy
  client again. See `docs/design/identity.md` and
  `docs/decisions/0008-identity-and-token-model.md`.

- New: **inbound local delivery** — received mail now files into the account
  store with **Sieve at the boundary**, closing the SMTP → mailbox path
  (previously inbound mail terminated at a spool). On the MX role with a
  database configured, each `RCPT TO:` for a hosted domain is resolved against
  the store (`Store::account_by_email`, subaddress-aware): an **unknown local
  user is refused `550 5.1.1` at RCPT** (an honest immediate answer, never a
  silent drop or post-DATA backscatter), while the anti-open-relay guard still
  refuses non-local recipients to unauthenticated senders. At end of `DATA` the
  fully-stamped message (Received + Authentication-Results + body) is delivered
  to **each** resolved recipient through `AccountStore::deliver_sieve` (parse →
  spam score → Sieve → file), isolation inherited per recipient. Sieve
  `redirect`/`vacation` actions are enqueued through the existing outbound queue
  under the rule owner's identity, with all attacker-influenced header strings
  (`subject`/`from`/redirect address) **CR/LF-stripped before any header is
  built**, and the store's redirect-rate budget enforced on the real path.
  Delivery is **per-recipient, try-then-commit**: a transient store/blob fault
  yields a conservative whole-message `4xx` so the sender retries (RFC 5321 §6.1
  — **duplicate delivery is preferred to loss**; blobs dedup by content), and
  **no failure path loses mail**. Delivered bytes go to a **durable on-disk blob
  backend** (`BlobStore::local`, `FICINA_SMTP_BLOB_DIR`, default `./blobs`), so a
  body survives a restart on single-node deployments without Garage/S3. The
  inbound **spool is retired as the local sink**: its all-local backlog is
  migrated into the store once at startup (before the queue runner claims), and
  it remains the outbound queue's durable store (unchanged). Reviewed +
  security-audited. See `docs/design/local-delivery.md` and the new inbound
  entries in `docs/interop.md`.

- New: **`ficina-sieve`** + delivery-time filtering — user **Sieve** filter
  scripts (RFC 5228, with **vacation** RFC 5230, **subaddress** RFC 5233,
  **imap4flags** RFC 5232) compiled and run on the server at delivery time.
  Sieve scripts are user-supplied programs, so every limit is a security
  control: hard parse caps (script size, nesting depth, test-list length,
  string size) enforced *during* parse, an evaluation instruction budget,
  and `require` enforcement (an un-declared extension is a compile error).
  Actions keep/fileinto/discard/redirect/stop with **implicit keep**, and
  **no script failure ever loses mail** — a compile error, a budget overrun,
  or a `fileinto` to a non-existent folder (auto-create is off) all fall back
  to implicit keep. **Redirect storms are impossible by construction**
  (per-script count cap, per-account rolling rate budget, loop guards,
  self-redirect refusal) and **vacation** carries the full RFC 3834 backscatter
  guards plus per-correspondent `:days` suppression. Wired at the store's
  delivery entry (`AccountStore::deliver_sieve`, after spam scoring and before
  filing); scripts, suppression, and the redirect budget are per-account rows,
  so isolation is inherited (cross-tenant **and** cross-account CRUD and
  execution tested). **Rule management is JMAP for Sieve** (RFC 9661, ADR
  0007): `SieveScript/{get,set,validate}` compile-checked on `set`
  (`invalidScript`), with the sieve capability in the Session resource.
  Reviewed + security-audited. The `deliver_sieve` seam is now exercised on the
  real inbound path (see "inbound local delivery" above). See
  `docs/design/sieve-filtering.md` and `docs/decisions/0007-sieve-rule-management.md`.

- New: **`ficina-imap`** — IMAP4rev2 (RFC 9051) / IMAP4rev1 (RFC 3501) and
  POP3 (RFC 1939) **compatibility shims** over the account store, so the
  installed base of mail clients (Thunderbird, Apple Mail, Outlook, phones
  over IMAP) can reach a Ficina mailbox unchanged. JMAP stays the native
  protocol (ADR 0001); these are thin translators over `AccountStore`, so
  tenant/account isolation is **inherited**, not re-implemented. IMAP on
  implicit TLS (993) and STARTTLS (143), POP3 on implicit TLS (995);
  `LOGIN`/`AUTHENTICATE PLAIN`/`LOGIN` are refused before TLS (no
  credentials in the clear) and both protocols cap failed authentications
  per connection. Full command set: `SELECT`/`EXAMINE`, `LIST`/`LSUB`
  (correct `%`/`*` wildcards + RFC 6154 special-use), `CREATE`/`DELETE`/
  `RENAME`, `STATUS`, `APPEND` (through the **same** ingestion path as
  delivery — no second parser), `FETCH` (`ENVELOPE`, `INTERNALDATE`,
  `RFC822.SIZE`, `FLAGS`, byte-exact `BODY[]`/`[HEADER]`/`[TEXT]`/
  `[HEADER.FIELDS]`/numbered parts with `<partial>`, and a bounded-honest
  `BODYSTRUCTURE`), `STORE`, `SEARCH`, `EXPUNGE`, `COPY`/`MOVE` (RFC 6851,
  with `COPYUID`/`APPENDUID`), every `UID` variant, and `IDLE` (RFC 2177)
  as **account-scoped push** off the per-account change cursor.
  **Stable per-mailbox UIDs and UIDVALIDITY** (schema migration 0006):
  strictly-ascending, never reused within an epoch, stable across
  reconnection; `EXPUNGE` renumbers sequence numbers, never UIDs. Covered
  by a cross-tenant **and** cross-account isolation suite plus UID-
  stability, concurrent-session, malformed/oversized-input, pipelining,
  STARTTLS, and POP3 integration tests over real TLS; reviewed and
  security-audited. `CONDSTORE`/`QRESYNC`, `SORT`/`THREAD`, `ACL`/`QUOTA`/
  `METADATA`, and sub-second IDLE via `LISTEN`/`NOTIFY` are additive
  follow-ups. See `docs/design/imap-pop3-shims.md`.

- Fixed: **account-scoped change visibility** — the JMAP/IMAP state cursor
  is now a **per-account** monotonic modseq (`account_modseq`, migration
  0005), not per-tenant, so a co-tenant user's activity can no longer
  advance another user's state token (closing a coarse activity-volume
  side channel and removing a spurious cross-account push wakeup). The
  change log was already per-account; only the counter was shared. State
  tokens stay opaque; `/changes` resumes unchanged.

- New: **`ficina-jmap`** — the JMAP API (RFC 8620 core, RFC 8621 mail),
  an HTTP service over the store and Ficina's native client protocol.
  **A public contract from merge** (web/desktop/compat adapters speak
  it): the Session resource with honest, enforced limits; the
  Request/Response envelope with ordered method dispatch and result
  references (back-references); `Mailbox`, `Email`, and `Thread`
  `get`/`set`/`query`/`changes` mapped onto the store; blob
  upload/download (blob ids are the store's — one id space; download is
  tenant-scoped, served with the stored Content-Type and `nosniff`); and
  an EventSource push endpoint emitting `StateChange` per tenant with
  heartbeats. `/changes` is backed by a new per-tenant monotonic modseq
  and change log in the store (`ficina-store::changes`), with opaque
  state tokens and an honest `cannotCalculateChanges`. **Interim bearer
  auth** (`/auth/token`, argon2 credentials in the store) resolves each
  token to `(tenant, account)` and enters the store only through
  `for_account` — behind a seam the future ficina-identity OIDC replaces
  without touching method code. Isolation is **per-account** (accountId =
  user): every by-id read/mutate, `/changes`, `Thread/get`, and blob
  download is scoped to the token's `(tenant, user)`, so a user cannot
  reach another user's mail even within the same tenant. Covered by the
  wrong-tenant AND cross-account isolation suites (CI-gated), plus
  conformance, result-reference, concurrent-`/changes`, `/changes`
  pagination-group, and malformed/oversized-body tests, all against real
  Postgres.
  `EmailSubmission/set` (send), full MIME `bodyStructure`, and
  JMAP-over-WebSocket are follow-ups. See `docs/design/jmap-api.md`.

- New: **`ficina-store`** — the account-scoped message store on
  PostgreSQL (system of record, via `sqlx` with compile-checked queries)
  and Garage/S3 (message bytes). **Isolation is structural, enforced by
  the type you hold:** user-owned mail data is reachable only through an
  `AccountStore`, obtained via `Store::for_account(TenantId, UserId)`,
  and every query bakes in its `(tenant, user)` predicate by construction
  — no API takes a `tenant_id` or `user_id` parameter, there is no
  ownership guard in any call path to forget, and a wrong-tenant *or*
  wrong-account lookup returns a clean `NotFound` (no cross-account
  oracle). Tenant-level provisioning (users, credentials) stays on a
  narrow `TenantStore` from `Store::for_tenant(TenantId)`. Entities: tenants, users, hierarchical mailboxes (with
  transactional total/unread counters), messages (with the parsed
  `Authentication-Results` verdict stored queryable), threads (RFC 8621
  §3 References-based), message↔mailbox membership, JMAP keywords/flags,
  and content-addressed blobs (SHA-256, per-tenant key prefix,
  ref-counted for a later GC sweep). Ids are opaque and random — no
  sequential integer crosses the API boundary. Ingestion writes the blob
  before the DB commit, so a crash leaves an invisible orphan (GC'd),
  never a visible message with a missing body. Full-text search
  (Postgres `tsvector`) over subject/addresses/body, updated in the same
  transaction as ingestion. Every list path is bounded by a `Page`. The
  Garage S3 backend is behind the `garage` cargo feature; tests use an
  in-memory backend. A **wrong-tenant and cross-account isolation suite**
  covers every public read and write path — proving two users of the same
  tenant cannot reach each other's rows with no guard in the path — and is
  required by CI, alongside threading
  property tests, concurrent-counter tests, and ingestion crash-safety
  tests (all against real Postgres). JMAP/IMAP endpoints, the Garage
  live-integration test, and the spool-migration tool are follow-ups.

- New: **Rspamd spam scoring** at DATA and **MTA-STS** policy serving
  (Phase 1 M4b), finishing M4's deferrals. On the MX role, after
  SPF/DKIM/DMARC, `ficina-smtp` consults Rspamd over `POST /checkv2`
  (`FICINA_SMTP_RSPAMD_URL`): a `reject` action refuses with **550**,
  `soft reject`/`greylist` defer with **451**, and otherwise the message
  is accepted with the score recorded as an `x-spam` method in
  `Authentication-Results`. A scanner that is unreachable, slow, or
  answers unparseably **fails closed** (451) — configuring a scanner and
  having it down never silently disables filtering. Scanning is off
  until the URL is set (`FICINA_SMTP_RSPAMD_TIMEOUT_SECS` bounds the
  call). **MTA-STS** (RFC 8461): the policy (`mode`/`mx`/`max_age`, with
  a content-derived `id`) is rendered from config and served at
  `GET /.well-known/mta-sts.txt` on `FICINA_SMTP_MTA_STS_ADDR` (plaintext
  behind the deploy TLS proxy); knobs `FICINA_SMTP_MTA_STS_MODE/MX/
  MAX_AGE/ID`, with the `_mta-sts` and `mta-sts` DNS records documented
  in `docs/interop.md`. ARC, TLS-RPT reporting, and DMARC report
  delivery remain deferred (see ROADMAP).

- New: `ficina-auth-mail` — the email-authentication trust stack (Phase
  1 M4), wired into `ficina-smtp`. Inbound (MX) at DATA: **SPF** (RFC
  7208 full `check_host` with macro expansion and the 10-DNS-lookup /
  2-void-lookup hard limits), **DKIM** verification (RFC 6376 + Ed25519
  per RFC 8463; relaxed/simple canonicalization, `l=`/`x=`, multiple
  signatures), and **DMARC** (RFC 7489; public-suffix org-domain,
  relaxed/strict alignment, `p=reject` → 550, with `pct=` sampling per
  §6.6.4 so a sender mid-rollout is not enforced at 100%). Every verdict
  is recorded in **`Authentication-Results`** (RFC 8601) — the public
  contract downstream parses — plus a `Received-SPF` header; any
  pre-existing `Authentication-Results` bearing our authserv-id (and any
  `Received-SPF`) is stripped from inbound mail first (RFC 8601 §5) so a
  remote sender cannot forge the verdict. A DKIM signature whose `h=`
  omits `From` is a permfail (RFC 6376 §6.1.1). Outbound
  (submission): **DKIM signing** with RSA-2048 or Ed25519, keys
  addressed by `(domain, selector)` behind a `KeyStore` (file backend
  with permission checks and zeroizing buffers) so rotation is a config
  change. RSA uses `ring` (constant-time), not the `rsa` crate
  (RUSTSEC-2023-0071). New knobs: `FICINA_SMTP_DKIM_DOMAIN/SELECTOR/KEY/
  ALGORITHM`. DMARC report delivery, ARC, MTA-STS, TLS-RPT, and Rspamd
  are deferred (see ROADMAP).

- New: `ficina-smtp` TLS and authenticated submission (Phase 1 M3).
  **STARTTLS** (RFC 3207) on the MX and submission ports and **implicit
  TLS** (port 465), via rustls with the ring provider — pure Rust, no
  OpenSSL. A PEM certificate/key is loaded from disk
  (`FICINA_SMTP_TLS_CERT`/`FICINA_SMTP_TLS_KEY`) or a self-signed one is
  generated for development. **AUTH PLAIN and LOGIN** (RFC 4954),
  offered only on a submission port over active TLS; wrong password and
  unknown user are indistinguishable (535, anti-enumeration).
  **Submission listeners** (`FICINA_SMTP_SUBMISSION_ADDR` for STARTTLS,
  `FICINA_SMTP_IMPLICIT_TLS_ADDR` for 465) require authentication before
  MAIL (530) — closing the open-relay hole ahead of enabling outbound.
  Credentials come from `FICINA_SMTP_CREDENTIALS_FILE` (a dev bootstrap;
  ficina-identity replaces it in M9). **RFC 6409** submission fixups add
  a `Date:` and `Message-ID:` when absent. EHLO now advertises a
  truthful capability set (STARTTLS/AUTH/SIZE/8BITMIME) reflecting the
  connection's exact state, and MAIL accepts `SIZE=`/`BODY=`/`AUTH=`
  parameters for the advertised extensions. `Received:` records
  `ESMTPS` for TLS-protected sessions (RFC 3848).
- New: `ficina-smtp` outbound delivery (Phase 1 M2) — a durable queue
  over the spool relays accepted mail. MX resolution (RFC 5321 §5.1:
  preference order, implicit MX, RFC 7505 null-MX = permanent),
  outbound SMTP client with RFC 5321 §4.5.3.2 timeouts and
  dot-stuffing, exponential backoff with jitter (4xx transient vs 5xx
  permanent), per-recipient durable state so a partial delivery never
  re-sends to already-delivered recipients, and RFC 3464 DSN bounces
  from the null sender (never bouncing a null-sender message, §4.5.5).
  **Relay safety: outbound is OFF by default** — enabled only via
  `FICINA_SMTP_OUTBOUND_ENABLED=true`, because open relaying must wait
  for the AUTH gate (M3). `FICINA_SMTP_SMARTHOST` routes all mail to
  one host (self-hosted mode). Knobs: `FICINA_SMTP_RETRY_BASE_SECS`,
  `FICINA_SMTP_RETRY_CAP_SECS`, `FICINA_SMTP_MAX_ATTEMPTS`,
  `FICINA_SMTP_QUEUE_INTERVAL_SECS`. Domainless recipients (bare
  `postmaster`) are parked pending local delivery (M5), never dropped.
- New: `ficina-smtp` receives mail end-to-end (Phase 1 M1) — full
  MAIL FROM / RCPT TO / DATA transactions with RFC 5321 sequencing
  (503 on out-of-order commands), address parsing incl. quoted local
  parts, address literals, source routes, the null sender and
  `<postmaster>`; DATA with dot-unstuffing, the size limit enforced
  during read (552), and bare-line-ending rejection (SMTP-smuggling
  defense); a `Received:` header stamped on every accepted message;
  durable maildir-style spool (`FICINA_SMTP_SPOOL_DIR`) with fsync +
  atomic-rename commit. New knobs: `FICINA_SMTP_MAX_MESSAGE_SIZE`
  (default 25 MiB), `FICINA_SMTP_MAX_RCPT` (default 100). HELO, RSET,
  NOOP, VRFY (252, anti-enumeration), HELP/EXPN → 502.
- New: `ficina-smtp` service — accepts TCP connections on port 2525,
  greets with a 220 banner, and answers EHLO and QUIT with
  RFC 5321-correct replies. Enforces the 512-octet command-line limit
  during read, rejects bare-LF line endings (SMTP-smuggling defense),
  and closes idle sessions after 5 minutes with 421. Configuration:
  `FICINA_SMTP_ADDR`, `FICINA_SMTP_HOSTNAME`. `--healthcheck` flag
  probes a running instance for container health.
- New: `deploy/docker-compose.yml` — the pinned engine set (Synapse
  v1.157.1, LiveKit v1.13.4, Collabora CODE 25.04.9.4.1, Garage
  v2.3.0, PostgreSQL 16.14, Rspamd 4.1.2) plus ficina-smtp, with
  healthchecks and `.env.example`.
- New: `scripts/fetch-engines.sh` — clones engine sources into
  `../engines` (read-only reference) at exactly the compose-pinned
  versions.
- New: CI runs the quality gate on every PR; releases build from tags
  only.
