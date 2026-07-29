#!/usr/bin/env bash
# Ficina encrypted backup: database + message blobs + TLS certs + config/DKIM.
#
# Uses restic (encrypted, deduplicated). The repository password lives in
# /root/.config/ficina/restic-password (0600, NEVER in the repo). Installed to
# /opt/ficina/backups/backup.sh and run daily by ficina-backup.timer.
#
# Restore is documented in docs/operations-runbook.md ("Restore from backup").
set -euo pipefail
export RESTIC_REPOSITORY=/opt/ficina/backups/restic
export RESTIC_PASSWORD_FILE=/root/.config/ficina/restic-password
STAGING=/opt/ficina/backups/staging
cd /opt/ficina/deploy/production
mkdir -p "$STAGING"

# 1. Database (custom compressed dump).
docker compose exec -T postgres pg_dump -U ficina -d ficina -Fc > "$STAGING/ficina-db.dump"

# 2. Docker volume paths (message bodies + TLS certs).
BLOBS=$(docker volume inspect ficina_blobs -f "{{.Mountpoint}}")
CERTS=$(docker volume inspect ficina_certs -f "{{.Mountpoint}}")

# 3. Encrypted, deduplicated backup of everything that matters.
restic backup --tag ficina \
  "$STAGING/ficina-db.dump" \
  "$BLOBS" \
  "$CERTS" \
  /opt/ficina/deploy/production

# 4. Retention: daily for a week, weekly for a month; prune the rest.
restic forget --tag ficina --keep-daily 7 --keep-weekly 4 --prune
