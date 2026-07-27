# Changelog

User- and operator-visible changes, written when the knowledge is
fresh (release skill). Versions follow SemVer against public
contracts.

## Unreleased

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
