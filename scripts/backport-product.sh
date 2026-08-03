#!/usr/bin/env bash
# Generic back-port (ADR 0019 two-way sync): the changes made in a product's
# public repo *past the last publish* (external contributions), for the paths
# shared verbatim with the monorepo — platform, the product's crates, migrate,
# and web source (not the transformed Cargo.toml / web build config / deploy /
# licence / README / CI, which the export owns).
#
# Usage: backport-product.sh <product> <product-clone-dir> <output-patch>
set -euo pipefail

PRODUCT="${1:?usage: backport-product.sh <product> <clone> <output-patch>}"
CLONE="${2:?product clone dir required}"
PATCH="${3:?output patch path required}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=/dev/null
source "$ROOT/scripts/products/$PRODUCT.sh"
: "${PRODUCT_CRATE_DIR:?config must set PRODUCT_CRATE_DIR}"

BASE="$(git -C "$CLONE" log --grep='sync from alo-workplace' -n1 --format=%H 2>/dev/null || true)"
if [ -z "$BASE" ]; then
  echo "backport: no publish marker in $PRODUCT history; nothing to do" >&2
  : > "$PATCH"; exit 0
fi
HEAD="$(git -C "$CLONE" rev-parse HEAD)"
if [ "$BASE" = "$HEAD" ]; then
  echo "backport: $PRODUCT has no commits past the last publish; nothing to do"
  : > "$PATCH"; exit 0
fi

git -C "$CLONE" diff --binary "$BASE..$HEAD" -- \
  platform "$PRODUCT_CRATE_DIR" migrate web \
  ':(exclude)web/vite.config.ts' ':(exclude)web/tsconfig.json' \
  > "$PATCH"

if [ -s "$PATCH" ]; then
  echo "backport: $PRODUCT patch written ($(wc -l < "$PATCH") lines) from ${BASE:0:8}..${HEAD:0:8}"
else
  echo "backport: $PRODUCT — only non-shared files changed; nothing to back-port"
fi
