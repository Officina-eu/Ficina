#!/usr/bin/env bash
# run-loop.sh — the Business-track build loop engine (docs/autonomy/LOOP.md),
# macOS/Linux version. One Claude Code invocation per queue item, forever —
# until QUEUE.md is complete (STATE.md gains "LOOP COMPLETE") or the loop
# halts ("LOOP HALT"). Ctrl+C is always safe: every finished item was already
# committed and pushed by the iteration that built it.
#
# Usage:
#   bash scripts/run-loop.sh [repo-path]     # default: the repo containing this script
set -u

REPO="${1:-$(cd "$(dirname "$0")/.." && pwd)}"
TRACK="${2:-business}"                    # business | sites (LOOP.md Tracks table)
MAX_ITERATIONS="${MAX_ITERATIONS:-500}"   # hard backstop against runaway loops
PROMPT="Read docs/autonomy/LOOP.md and execute exactly ONE iteration of the loop for track '$TRACK', then exit."
STATE_FILE="docs/autonomy/STATE.md"
[ "$TRACK" = "sites" ] && STATE_FILE="docs/autonomy/sites/STATE.md"

cd "$REPO"

for ((i = 1; i <= MAX_ITERATIONS; i++)); do
  echo "============================================================"
  echo "[loop] iteration $i  $(date '+%Y-%m-%d %H:%M')"

  git pull --rebase origin main >/dev/null 2>&1

  state="$(cat "$STATE_FILE" 2>/dev/null || true)"
  if grep -q "LOOP COMPLETE" <<<"$state"; then
    echo "[loop] queue complete — stopping."; break
  fi
  if grep -q "LOOP HALT" <<<"$state"; then
    echo "[loop] halted by the agent — fix the reason in STATE.md, remove the marker, restart."; break
  fi

  # One iteration. --dangerously-skip-permissions is required for unattended
  # runs; the hard safety rails live in LOOP.md and the repo's deny rules.
  claude -p "$PROMPT" --dangerously-skip-permissions
  code=$?

  if [ "$code" -ne 0 ]; then
    # Rate limit / transient failure: back off instead of spinning.
    echo "[loop] iteration exited with code $code — waiting 15 minutes."
    sleep 900
  else
    sleep 10
  fi
done
echo "[loop] done."
