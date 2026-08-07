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

# SINGLE-WRAPPER LOCK: three concurrent-editor incidents traced to stopped
# wrappers surviving as detached processes and spawning rival workers. A
# wrapper now claims its track machine-wide and refuses to start if a live
# owner exists; a stale lock (dead PID) is taken over.
LOCK="$HOME/.alo-loop-$TRACK.lock"
if [ -f "$LOCK" ]; then
  oldpid="$(cat "$LOCK" 2>/dev/null)"
  if [ -n "$oldpid" ] && kill -0 "$oldpid" 2>/dev/null; then
    echo "[loop] another wrapper (PID $oldpid) owns track '$TRACK' on this machine — refusing to start."
    echo "[loop] if that wrapper is truly dead, remove $LOCK and retry."
    exit 3
  fi
  echo "[loop] stale lock from dead PID $oldpid — taking over."
fi
echo $$ > "$LOCK"
trap 'rm -f "$LOCK"' EXIT

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

  # One iteration, with an IDLE-based hang guard: a truly hung worker goes
  # silent (its session transcript stops growing) while an honest long item
  # keeps writing — a duration-only guard once executed 90 min of honest
  # work. Kill after IDLE_KILL_MIN of transcript silence; the ceiling is the
  # absolute backstop. The killed item is redone next iteration.
  # --dangerously-skip-permissions is required for unattended runs; the hard
  # safety rails live in LOOP.md and the repo's deny rules.
  proj_key="$(printf '%s' "$REPO" | sed 's#[/: ]#-#g')"
  transcripts="$HOME/.claude/projects/$proj_key"
  start_epoch=$(date +%s)

  claude -p "$PROMPT" --dangerously-skip-permissions &
  cpid=$!
  idle_kill=$(( ${IDLE_KILL_MIN:-20} * 60 ))
  ceiling=$(( ${ITERATION_CEILING_MIN:-240} * 60 ))
  code=""
  while kill -0 "$cpid" 2>/dev/null; do
    sleep 30
    now=$(date +%s)
    # Newest transcript write overall (macOS stat -f, GNU stat -c fallback).
    # The worker creates/writes its transcript within seconds of starting, so
    # newest-overall tracks the CURRENT session almost immediately.
    # GNU stat (-c) FIRST: on Git Bash, BSD-style `stat -f %m` prints
    # filesystem info instead of failing, poisoning the age check — the guard
    # then killed honestly-working workers every 20 minutes. macOS falls
    # through to -f when -c errors out. Numeric-only guard on the result.
    newest=$(find "$transcripts" -name '*.jsonl' -exec stat -c %Y {} \; 2>/dev/null | grep -E '^[0-9]+$' | sort -rn | head -1)
    if [ -z "$newest" ]; then
      newest=$(find "$transcripts" -name '*.jsonl' -exec stat -f %m {} \; 2>/dev/null | grep -E '^[0-9]+$' | sort -rn | head -1)
    fi
    if [ -z "$newest" ] || [ "$newest" -lt "$start_epoch" ] 2>/dev/null && [ $(( start_epoch + 120 )) -gt "$now" ]; then
      # First two minutes: give the worker time to open its transcript.
      newest=$start_epoch
    fi
    [ -z "$newest" ] && newest=$start_epoch
    idle=$(( now - newest ))
    reason=""
    if [ "$idle" -ge "$idle_kill" ]; then reason="silent for $((idle / 60)) min"; fi
    if [ $(( now - start_epoch )) -ge "$ceiling" ]; then reason="hit the $((ceiling / 60))-min ceiling"; fi
    if [ -n "$reason" ]; then
      echo "[loop] killing the worker — $reason."
      kill -TERM "$cpid" 2>/dev/null; sleep 10; kill -KILL "$cpid" 2>/dev/null
      # Drop half-done uncommitted state so the next iteration starts clean.
      git rebase --abort >/dev/null 2>&1
      git checkout -- . >/dev/null 2>&1
      code=124
      break
    fi
  done
  if [ -z "$code" ]; then
    wait "$cpid"; code=$?
  else
    wait "$cpid" 2>/dev/null || true
  fi

  if [ "$code" -eq 124 ]; then
    sleep 30                          # the hang already wasted time — go again
  elif [ "$code" -ne 0 ]; then
    # Rate limit / transient failure: back off instead of spinning.
    echo "[loop] iteration exited with code $code — waiting 15 minutes."
    sleep 900
  else
    sleep 10
  fi
done
echo "[loop] done."
