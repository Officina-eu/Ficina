# M3 — TLS + Submission (design note)

Phase 1 milestone M3 for `ficina-smtp`. Adds transport security and
authenticated submission on top of the M1 receive path and M2 queue.

## Surface

Three listener **roles**, each a bound address with a TLS mode:

| Role | Default dev port | TLS mode | AUTH | Purpose |
|---|---|---|---|---|
| `Mx` | 2525 | STARTTLS offered | never | Inbound relay (port 25). No sender auth. |
| `Submission` | 2587 | STARTTLS **required before MAIL** | required | MSA (port 587), RFC 6409. |
| `ImplicitTls` submission | 2465 | TLS from first byte | required | Submissions (port 465). |

New protocol surface:

- **EHLO capabilities** become a truthful multiline reply (RFC 5321
  §4.1.1.1, §4.2.1). Advertised: `STARTTLS` (when TLS is available and
  not already active), `AUTH PLAIN LOGIN` (submission + TLS active
  only), `SIZE <max>`, `8BITMIME`. (We do not advertise
  `ENHANCEDSTATUSCODES`: enhanced codes appear on the TLS/AUTH replies
  but not uniformly on every reply, so advertising it would overclaim.)
- **STARTTLS** (RFC 3207): `220`, then the server performs the TLS
  handshake and the session resets — the client MUST send EHLO again.
- **AUTH** (RFC 4954) `PLAIN` and `LOGIN` — offered only on a
  submission role over an active TLS connection.
- **Submission rewrites** (RFC 6409 §8): a `Date:` and a `Message-ID:`
  header are added when absent. `Received:` is already stamped (M1).

## Errors (each maps to a specific reply)

- STARTTLS with no TLS configured → `454 4.7.0 TLS not available`.
- STARTTLS when TLS already active → `503` (plain bad-sequence text).
- **STARTTLS injection defense** (RFC 3207 §5): any buffered client
  octet present after the `220` and before the handshake means the
  client pipelined plaintext across the TLS boundary — the connection
  is dropped, nothing is processed.
- AUTH on an `Mx` role → `503` (AUTH not offered on this port).
- AUTH without active TLS → `538 5.7.11 encryption required for AUTH`.
- AUTH when already authenticated → `503` (already authenticated).
- Bad credentials → `535 5.7.8`. Malformed SASL/base64 → `501 5.5.2`.
- On a submission role: MAIL before STARTTLS → `530 5.7.0 must issue
  STARTTLS first`; MAIL before successful AUTH → `530 5.7.1
  authentication required`.

## Tenancy

Still no store (that is M5), so no tenant-scoped reads/writes yet. But
AUTH introduces **identity**: a successful login records an
`AuthIdentity` on the session. Credentials never appear in logs,
errors, or the spool. This is the seam the tenant model plugs into —
the `Authenticator` trait is config-backed now and becomes
`ficina-identity` in M9.

## Contracts

- Config gains listener specs (addr + role + tls mode), a TLS
  cert/key source, and a credential source — all additive.
- `Authenticator` is a trait so M9 swaps the backend without touching
  the SMTP code.
- The EHLO reply moving from single- to multi-line is additive: every
  client already parses multiline replies (§4.2.1).

## Out of scope (recorded, deferred)

- DKIM/SPF/DMARC — the M4 trust stack.
- AUTH mechanisms beyond PLAIN/LOGIN (no CRAM-MD5/SCRAM/XOAUTH2):
  PLAIN+LOGIN over TLS is the universal client baseline; others land
  only if a real client in the interop matrix needs them.
- RFC 6409 `From`/`Sender` rewriting and address canonicalization — we
  do the safe, non-destructive subset (Date, Message-ID) only.
- **Sender authorization** (binding `MAIL FROM` to the authenticated
  identity / a send-as permission model, RFC 6409 §6.1) — deferred to
  ficina-identity (M9): a strict "sender == login" rule breaks
  legitimate shared mailboxes and aliases, so it needs the real
  permission model, not an interim approximation.
- TLS client-certificate authentication.

## Security review — accepted deferrals

Fixed in this milestone (security audit + cold review): the MX
open-relay guard (`local_domains`), the self-signed-in-production gate
(`FICINA_SMTP_ALLOW_SELF_SIGNED`), a per-connection failed-AUTH cap,
control-character rejection in SASL identities, and keeping the login
name out of default-visible logs. Two LOW findings are deferred with
rationale:

- **Constant-time credential comparison.** `StaticAuthenticator` is the
  dev bootstrap; `ficina-identity` (M9) owns the real backend and must
  use a constant-time verify plus a dummy-hash path for unknown users
  so wrong-password and unknown-user are timing-indistinguishable.
- **Per-source-IP connection limit.** Today one IP can occupy the whole
  per-listener connection pool. A per-peer cap belongs with the
  gateway/rate-limiting layer; recorded for the deliverability/ops
  hardening pass, not launch-blocking.

## Rejected alternative

A single port that infers submission-vs-relay from whether the client
authenticated. **Rejected:** port-based roles (25/587/465) are the
deployed convention every firewall, MTA, and mail client already
depends on, and collapsing relay and submission onto one port muddies
the AUTH-required and open-relay boundaries that keep us safe. Explicit
per-port roles keep the security policy legible and auditable.

## Library choice

`rustls` (with the `ring` provider) + `tokio-rustls`, pure Rust — no
OpenSSL/C dependency, consistent with "Rust below the waterline" and
easier to audit. `rcgen` generates a self-signed certificate for
dev/test when none is configured; PEM cert+key files are loaded in
production. **Rejected:** `native-tls`/OpenSSL — pulls a C library
against the Rust-only doctrine and complicates the supply chain.
