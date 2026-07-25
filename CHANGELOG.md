# Changelog

User- and operator-visible changes, written when the knowledge is
fresh (release skill). Versions follow SemVer against public
contracts.

## Unreleased

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
