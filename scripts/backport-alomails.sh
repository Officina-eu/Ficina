#!/usr/bin/env bash
# Compute the back-port patch (ADR 0019 two-way sync): the changes made in the
# public alomails repo *on top of the last publish* — i.e. external
# contributions — for the paths alomails shares **verbatim** with this monorepo.
#
# The monorepo stays the source of truth; contributions land in alomails, and
# this turns them into a reviewable patch for the monorepo. The last publish
# commit (written by the publish pipeline, marker "sync from alo-workplace") is
# the base: everything after it on alomails main is a contribution.
#
# Only verbatim-shared paths are back-ported. The transformed / alomails-only
# files — Cargo.toml, the web build config (vite/tsconfig), deploy/, LICENSE,
# README, CI — are OWNED by the export and a maintainer handles them by hand;
# blindly pulling them back would drag the mail-only defaults into the monorepo.
#
# Usage: backport-alomails.sh <alomails-clone-dir> <output-patch>
set -euo pipefail
ALO="${1:?usage: backport-alomails.sh <alomails-clone> <output-patch>}"
PATCH="${2:?output patch path required}"

# The last commit the publish pipeline wrote is the divergence point.
BASE="$(git -C "$ALO" log --grep='sync from alo-workplace' -n1 --format=%H 2>/dev/null || true)"
if [ -z "$BASE" ]; then
  echo "backport: no publish marker in alomails history; nothing to do" >&2
  : > "$PATCH"
  exit 0
fi
HEAD="$(git -C "$ALO" rev-parse HEAD)"
if [ "$BASE" = "$HEAD" ]; then
  echo "backport: alomails has no commits past the last publish; nothing to do"
  : > "$PATCH"
  exit 0
fi

# Verbatim-shared paths only; the web build config is transformed → excluded.
git -C "$ALO" diff --binary "$BASE..$HEAD" -- \
  platform products/mail migrate web \
  ':(exclude)web/vite.config.ts' ':(exclude)web/tsconfig.json' \
  > "$PATCH"

if [ -s "$PATCH" ]; then
  echo "backport: patch written ($(wc -l < "$PATCH") lines) from ${BASE:0:8}..${HEAD:0:8}"
else
  echo "backport: only non-shared files changed past the last publish; nothing to back-port"
fi
