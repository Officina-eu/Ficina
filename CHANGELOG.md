# Changelog

User- and operator-visible changes, written when the knowledge is
fresh (release skill). Versions follow SemVer against public
contracts.

## Unreleased

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
