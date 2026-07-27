# M3 security follow-ups (from the security audit)

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

## Doc drift fixed

- The design note listed `ENHANCEDSTATUSCODES` among advertised EHLO
  capabilities, but `session::capabilities()` does not emit it (we do
  not add enhanced codes to every reply). Removed from the note.
