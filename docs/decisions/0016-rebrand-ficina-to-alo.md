# ADR 0016 — Rebrand: Ficina → alo (alo workplace)

Status: accepted

## Context

The project was built under the name **Ficina**. The company/product has been
renamed. The umbrella brand is **alo** (lowercase wordmark); the suite is **alo
workplace** (two words in prose; `aloworkplace` only where spaces are impossible,
e.g. package or image identifiers). The AI assistant is **alo**. The repository
will be open-sourced, so the rename must be complete: no stray `ficina` strings
except where a change would rewrite history or endanger live data.

## Decision

Rename everything to `alo` / `alo workplace`:

- **Crates** `ficina-*` → `alo-*` (folders, package names, workspace members, all
  `use` paths, binary names). `identityctl` keeps its name.
- **Env vars** `FICINA_*` → `ALO_*`.
- **Docker** project, network, service/container names, image build contexts, and
  in-container users/paths (`/var/lib/alo`, `/var/spool/alo`) → `alo`.
- **Runtime identifiers**: the DNS domain-verification prefix `_ficina-verify` →
  `_alo-verify`; the email round-trip HTML attributes `data-ficina-*` →
  `data-alo-*`; the built-in AI provider kind `"ficina"` → `"alo"`; the browser
  refresh-token key `ficina.rt` → `alo.rt`.
- **Docs/governance/CI**: product prose, CLAUDE.md, skills/agents, ROADMAP,
  README, CI job names.

### What is deliberately NOT renamed

- **Applied migrations** (`core/alo-store/migrations/*.sql`). They are checksummed
  by `sqlx::migrate!` against the live database; editing them — even a comment —
  makes the service fail to start. Their `ficina` mentions (comments, and the
  `_ficina-verify` reference in 0013) are frozen historical text. New migrations
  use `alo`.
- **Past ADRs** (0001–0015). They record decisions as they were made; they are
  historical record, not living docs. This ADR supersedes their branding.
- **The live PostgreSQL database name and role** — see "Deferred" below.
- **Named Docker volumes' physical names** — pinned to the pre-rebrand
  `ficina_*` names (see below) so existing data is preserved.

### Data-preservation choices (live deployment)

A production deployment exists. To avoid orphaning state on the next redeploy:

- **Volumes** are pinned in `deploy/production/docker-compose.yml` with explicit
  `name: ficina_pg_data` (etc.), so the renamed `alo` compose project attaches to
  the volumes the old `ficina` project created rather than provisioning empty
  ones. The ops scripts (`backup.sh`, `monitor.py`) reference those pinned names.
- **Container uid/gid** is pinned to `10001` in the service Dockerfiles for
  determinism. Because the pre-rebrand images used an unpinned `--system` uid, the
  operator may need to `chown` the blob/spool volumes once at redeploy — the
  paste-ready command is in `docs/rename-migration.md`.

### Deferred (tech debt)

- **Renaming the PostgreSQL database and role** from `ficina` to `alo`. The live
  cluster's database is named `ficina` and owned by role `ficina`, both living in
  the `ficina_pg_data` volume. Renaming them requires an `ALTER`/dump-restore that
  is not worth the risk during a pure rebrand. `.env.example` now defaults to
  `alo` for **fresh** installs; the existing operator keeps `POSTGRES_USER=ficina`
  / `POSTGRES_DB=ficina` in their `.env` (see the migration doc).
  **Trigger to resolve:** a scheduled maintenance window, and no later than the
  open-sourcing cutover.

## Consequences

- `grep -ri ficina` after the rename is intentionally non-empty: the frozen
  migrations, the past ADRs, the volume-name pins + the operator note that
  explains them, `docs/rename-migration.md`, and the README history line.
- The operator must perform the server-side steps in `docs/rename-migration.md`
  at the next deliberate redeploy; nothing on the running server changes until
  then.
