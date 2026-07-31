# SMTP security audit follow-ups

The M3 security audit returned **no critical or high findings**; the
submission open-relay gate and the STARTTLS injection defense both
hold. It recorded gaps that MUST close before outbound delivery is
enabled in a real deployment or before production launch. They are
gated today by outbound being off by default, but that is a default,
not a code enforcement — so they are tracked here.

## Must-fix before enabling outbound / production

1. **MX local-recipient allowlist.** The MX (port 25) role accepts
   `RCPT TO:` for any domain and spools it. With outbound enabled this
   would relay to arbitrary externals — a classic open relay. Fix
   applied in M3: an optional hosted-domains allowlist
   (`FICINA_SMTP_LOCAL_DOMAINS`) on the MX `RCPT` path (550 for
   non-local recipients when set), plus a loud startup refusal to
   enable outbound while the MX allowlist is empty — so the safety is
   enforced in code, not by the outbound-off default.

2. **Bind submission `MAIL FROM` to the authenticated identity.** An
   authenticated user could otherwise send `MAIL FROM:<someone-else@…>`
   (envelope-sender spoofing). **Deferred to M9** (not M3): a strict
   "sender == login" rule breaks legitimate aliases and shared
   mailboxes, so it needs the real identity→address send-as permission
   model rather than an interim approximation. The security audit
   sanctioned deferral provided the gap is recorded — it is, here and
   in the design note. Until then, submission requires TLS + AUTH, so
   only authenticated users can send at all; they are simply not yet
   restricted in which envelope sender they use.

3. **Per-connection AUTH attempt cap + tarpit.** No limit on `AUTH`
   attempts per connection allows online password brute-force. Fix
   applied in M3: a per-connection failed-attempt cap that closes the
   connection after N failures; a growing delay/tarpit and any
   cross-connection accounting land with the M9 argon2 backend.

## Lower-severity, deferred

- **Constant-time password compare** — the dev `StaticAuthenticator`
  compares plaintext non-constant-time; M9's argon2 backend fixes this.
- **Credentials file permission check** — warn if the dev credentials
  file is group/world-readable.
- **Self-signed fallback** — a submission server silently falls back to
  a self-signed cert (peer-auth downgrade) if no cert is configured,
  guarded only by a `warn!`. Consider a hard failure or explicit
  `dev`-mode flag before production.

## Deferred refactor (reviewer, not required)

- **Extract the SASL wire dialog from `server.rs`** into its own module
  (`do_auth`/`collect_credentials`/`read_sasl_line`, ~110 lines). The
  cold review flagged `server.rs` as borderline on one-file-one-reason
  but explicitly did not require the split, recommending it "the first
  time this file is touched again." Tracked here so it is not lost.

## ficina-identity audit (identity milestone)

The identity security audit + cold review returned **one blocker**
(non-atomic refresh-token rotation defeating replay-chain revocation under
concurrency) and a set of lower findings. **Fixed in this milestone:** the
atomic guarded rotate (`rotate_refresh_token` now `UPDATE … WHERE
rotated_to IS NULL RETURNING`, a lost race → chain revoke); per-`(client,
username)` backoff added to the non-public `/auth/token` password grant
(was only on `/oauth/authorize`); a process-wide **semaphore bounding
concurrent argon2** hashes (memory-exhaustion DoS lever); ID-token signing
seeds **zeroized** in memory after use; the tenant-scoped-client guard at
`/oauth/authorize`; `email_of` DB errors propagated (no silently-dropped
claim); `Retry-After` on the 429; and stale roadmap-coded comments removed.

**Deferred with rationale (recorded, not launch-blocking for the founder
dogfood; must-close before broad multi-tenant / self-service exposure):**

- **TOTP per-time-step single-use.** A valid code is replayable within the
  ±1-step (~90 s) acceptance window (RFC 6238 §5.2 recommends one code per
  step). Bounded by TLS and not enforced on legacy protocols; needs a
  per-user `last_totp_step` column and a monotonic check. Follow-up.
- **`email_verified` claim.** Emitted `true` for any user with an email.
  Phase-1 accounts are **operator-provisioned** via `identityctl` (the
  operator vouches for the address), so it is not a self-asserted claim
  today — but it must become a real per-user verified flag before
  self-service signup, or be dropped. Follow-up.
- **argon2 param-change enumeration window.** The unknown-user dummy hash
  uses the *configured* params while a real verify uses the *stored* PHC
  params; if a deployment changes `FICINA_IDENTITY_ARGON2_*` after hashes
  exist, the two costs diverge until each account rehashes on next login —
  a transient, self-healing timing skew. A fixed decoy PHC captured at the
  stored baseline would close the window. Follow-up.
- **ID-token signing key at rest.** Stored as raw bytes in `signing_keys`
  (design-accepted: single-node DB is the trust boundary). Envelope/KMS
  encryption of the private key is an ops-hardening follow-up; the
  in-memory seed is now zeroized.
- **Global email/alias uniqueness.** `users.email` is unique per tenant
  only; `aliases.address` is globally unique. `account_by_email` refuses on
  a cross-tenant canonical-email collision (returns `None` — never
  misroutes), so it is a mail-*availability* footgun, not a leak, and is
  harmless while provisioning is operator-only. A global `lower(email)`
  uniqueness (reconciled with aliases) or an explicit collision policy is
  required before self-service provisioning. Follow-up.
- **argon2 off the async worker.** The semaphore bounds concurrency but
  argon2 still runs inline on the runtime worker (~a few hundred ms).
  Moving it to `spawn_blocking` is a latency-isolation follow-up.
- **Sender authorization (send-as).** Binding submission `MAIL FROM` to the
  authenticated identity is still deferred (see item 2 above) — it needs
  the group/alias permission model this milestone ships the data for.

## ficina-identity audit — second pass (deployment-readiness)

A fresh, independent security audit + cold review before considering
internet exposure found gaps the first pass missed. **Fixed in this pass:**

- **Swallowed revocation `Result`s (review blocker).** Five `let _ =
  …revoke…` calls discarded store errors on the RFC 6749 §10.4 replay
  defense and the RFC 7009 `/oauth/revoke` path — a failed revoke reported
  success while tokens stayed live. Now: a detected replay whose chain
  revocation fails returns `server_error` (fail closed) and logs a
  SOC-alertable `warn`; `/oauth/revoke` returns `503` on a store fault
  rather than a false `200`.
- **Legacy 2FA bypass (audit HIGH).** `authenticate_password` (used by
  IMAP/POP3/SMTP) did no second-factor check, so a TOTP user still had a
  password-only mailbox over those protocols. New `authenticate_legacy`
  **fails closed** for a TOTP-enabled account (indistinguishable refusal)
  and is wired into all three protocols; ADR 0008 + design note updated.
- **No per-account backoff on legacy protocols (audit MEDIUM).**
  `authenticate_legacy` now applies per-username exponential backoff across
  connections (on top of the per-connection caps).
- **OIDC signing key not provisioned on the server path (review).**
  `jmap::serve` now calls the idempotent `ensure_signing_key()` at startup
  (fail-fast), so a deployment no longer serves a broken `/oauth/jwks` or
  fails to sign ID tokens without an out-of-band CLI step.
- **JWKS loaded private seed material (audit LOW).** A dedicated
  `public_signing_keys()` query feeds the JWKS; private seeds never transit
  the public-key path.
- **No OAuth-boundary instrumentation (review).** Structured `tracing`
  events (no secrets) on auth failure, replay detection, and revoke.
- **Client-lookup / credential store faults masked as client/credential
  rejections (review nit).** A DB fault now returns `server_error` and does
  not record a rate-limit strike against the user.
- **`secret_hash` contract trap (review).** Documented at the store field
  that confidential-client secrets are stored-but-not-verified (public PKCE
  clients only) until confidential-client support lands.

**Still deferred with rationale (must-close before broad multi-tenant /
self-service / third-party-client exposure; not blocking a founder
dogfood):**

- **JMAP does not enforce token scope (audit MEDIUM).** `state::authenticate`
  grants any valid access token full mailbox access regardless of scope.
  Contained today because only first-party full-scope public clients are
  registered; a concrete `mail` scope check must land **before** any
  reduced-scope or third-party client is registered.
- **Automatic rehash-on-login (audit LOW).** ADR 0008 mentions transparent
  rehash when argon2 params are raised; not implemented. Params are fixed at
  deploy, so the divergence (and the dummy-hash timing skew) does not arise
  on a single fresh deployment; implement before changing params on a live
  system.
- **TOTP per-time-step single-use** and the earlier deferrals (email_verified
  flag, argon2 → `spawn_blocking`, key-at-rest KMS, global email/alias
  uniqueness, send-as) remain as recorded above.
- **IdP transport invariant.** `/oauth/authorize` accepts a password POST and
  relies on the front TLS proxy; the deployment must terminate TLS and set
  HSTS in front of it (a Stage-3 deployment invariant, not app code).

## Doc drift fixed

- The design note listed `ENHANCEDSTATUSCODES` among advertised EHLO
  capabilities, but `session::capabilities()` does not emit it (we do
  not add enhanced codes to every reply). Removed from the note.

## Email submission (JMAP EmailSubmission/set) — tracked residuals

From the security-auditor pass on the send path (docs/design/email-submission.md).
The HIGH (From-header spoofing) and the LOW items were fixed in the same change;
these two remain as tracked follow-ups:

- **Per-user / per-tenant send-rate quota.** The submission path caps recipients
  per message (100) but has no rate limit, so an authenticated user could drive
  sustained DKIM-signed outbound (spam / IP-reputation risk). Add a send-rate
  quota (reuse the `ficina-identity` rate limiter or a store-backed counter)
  before `submit` in `core/ficina-jmap/src/submission.rs`.
- **Internal submission listener network exposure.** The trusted no-auth
  listener binds `0.0.0.0:2526` on the shared `ficina` docker network, so a CVE
  in any co-networked container (rspamd/caddy/postgres) could pivot to a relay.
  It is not internet-reachable (unpublished port) and send-as is enforced in
  ficina-jmap, so this is defence-in-depth. Recommended fix: replace the TCP
  channel with a **Unix domain socket** shared only between ficina-jmap and
  ficina-smtp (eliminates the network surface); alternatively a dedicated
  internal network + interface-bound listener.

## Ficina Transfer (large-file share links) — v1 follow-ups

The share feature (`core/ficina-{store,jmap}/src/share.rs`) now **streams** both
upload and download and has **no size limit**; the sender chooses the expiry
window. Two of the original v1 limitations are resolved by that redesign; the
remainder are tracked here.

RESOLVED:

- **Streaming (was: buffered ≤ 100 MB).** Upload streams the request body
  through `object_store`'s multipart writer (`BlobStore::put_share_stream`) and
  download streams via `GetResult::into_stream()` → `Body::from_stream`
  (`BlobStore::get_share_stream`). Nothing is buffered whole, so there is no size
  ceiling and no memory-amplification vector; the concurrency semaphore was
  removed (it would only self-DoS long streams).
- **Blob reclamation (was: cannot safely delete).** Share files are no longer
  content-addressed — each is written under its own random key
  `<tenant>/share/<id>` (migration 0027 renamed the column to `object_key`), so
  a share's object is never shared with a message attachment. `sweep_expired_shares`
  now deletes the object on expiry with no data-loss risk.

STILL OPEN:

- **Capability token in the URL.** The link is `…/share/<raw-token>`, so the live
  token can land in reverse-proxy access logs and browser history (the DB stores
  only its hash). Inherent to the WeTransfer-style capability-URL model; the
  download sends `Cache-Control: no-store`. If stronger secrecy is wanted, exclude
  `/share/*` from access logging or move the token out of the path.
- **Per-IP rate limiting.** There is no rate-limit layer in ficina-jmap; the
  public share download relies on the unguessable token alone. A gateway/per-IP
  limit is a general hardening item, not specific to this path — more relevant now
  that the size is unbounded (an attacker with a link can pull a large file
  repeatedly).
- **Per-tenant storage quota on the share path.** The streaming upload bypasses
  `put_blob`'s `check_quota`, so shares don't count against a tenant's storage cap.
  A streaming quota check (or a periodic reconciliation) is the fix.
