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

## Doc drift fixed

- The design note listed `ENHANCEDSTATUSCODES` among advertised EHLO
  capabilities, but `session::capabilities()` does not emit it (we do
  not add enhanced codes to every reply). Removed from the note.
