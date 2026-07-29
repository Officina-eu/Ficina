#!/usr/bin/env bash
# send-alert.sh "subject" "body" — sends one alert email via the monitor's
# own send path (authenticated submission through the server itself, so no
# third-party service). Used by ficina-backup-failed.service.
set -euo pipefail
SUBJECT="${1:?subject}"; BODY="${2:?body}"
python3 - "$SUBJECT" "$BODY" <<'PY'
import sys
sys.path.insert(0, "/opt/ficina/monitoring")
import monitor
monitor.send_email(sys.argv[1], sys.argv[2])
PY
