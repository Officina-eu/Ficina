#!/usr/bin/env bash
# Generic product-repo export engine (ADR 0019). Exports one product from the
# monorepo into a self-contained, AGPL, standalone repo: the shared platform +
# the product's crates + the product-configured web + the product's deploy +
# repo furniture (LICENSE, README, CI). Deterministic; a guard fails the export
# if the suite or another product leaks in.
#
# Adding a product is a config file (scripts/products/<product>.sh) + a README
# (scripts/products/<product>/README.md) — the engine below is shared.
#
# Usage: export-product.sh <product> <output-dir>
set -euo pipefail

PRODUCT="${1:?usage: export-product.sh <product> <output-dir>}"
OUT="${2:?output dir required}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CONF="$ROOT/scripts/products/$PRODUCT.sh"
COMMON="$ROOT/scripts/products/common"
[ -f "$CONF" ] || { echo "export: no product config $CONF" >&2; exit 1; }

PYTHON=""
for p in python3 python; do "$p" --version >/dev/null 2>&1 && { PYTHON="$p"; break; }; done
[ -n "$PYTHON" ] || { echo "export: no working python found" >&2; exit 1; }

# Config-provided knobs (defaults a config may override).
PRODUCT_WEB_DELETE=()
PRODUCT_WEB_EXTRA=()
product_prepare_deploy() { :; }
# shellcheck source=/dev/null
source "$CONF"
: "${PRODUCT_REPO:?config must set PRODUCT_REPO}"
: "${PRODUCT_CRATE_DIR:?config must set PRODUCT_CRATE_DIR}"
: "${PRODUCT_WEB_SURFACE:?config must set PRODUCT_WEB_SURFACE}"

rm -rf "$OUT"
mkdir -p "$OUT"

# --- Rust: the shared platform + this product's crates + the migrator --------
cp -r "$ROOT/platform" "$ROOT/migrate" "$ROOT/.sqlx" "$ROOT/.cargo" "$OUT/"
mkdir -p "$OUT/$(dirname "$PRODUCT_CRATE_DIR")"
cp -r "$ROOT/$PRODUCT_CRATE_DIR" "$OUT/$PRODUCT_CRATE_DIR"
cp "$ROOT/Cargo.lock" "$ROOT/.gitignore" "$ROOT/.gitattributes" "$OUT/"

# workspace manifest narrowed to platform + this product (shared deps verbatim).
"$PYTHON" "$COMMON/gen-manifest.py" "$ROOT/Cargo.toml" "$PRODUCT_CRATE_DIR" > "$OUT/Cargo.toml"

# service Dockerfiles must not COPY the (absent) suite layer.
for f in "$OUT/$PRODUCT_CRATE_DIR"/*/Dockerfile; do
  [ -f "$f" ] && sed -i '/^COPY suite \.\/suite$/d' "$f"
done

# --- web: this product's surface (ADR 0019 seam) -----------------------------
mkdir -p "$OUT/web"
tar cf - -C "$ROOT/web" --exclude=node_modules --exclude=dist --exclude=.turbo . \
  | tar xf - -C "$OUT/web"
for d in "${PRODUCT_WEB_DELETE[@]}"; do rm -rf "$OUT/web/src/$d"; done
for d in "${PRODUCT_WEB_EXTRA[@]}"; do rm -rf "$OUT/web/$d"; done
# Keep only this product's surface (plus the shared/types/index seam files).
find "$OUT/web/src/product" -maxdepth 1 -name '*.tsx' \
  ! -name "$PRODUCT_WEB_SURFACE.tsx" ! -name 'shared.tsx' -delete
sed -i -E "s#const product = .*#const product = \"$PRODUCT_WEB_SURFACE\";#" "$OUT/web/vite.config.ts"
sed -i -E "s#\"\./src/product/[a-zA-Z]+\.tsx\"#\"./src/product/$PRODUCT_WEB_SURFACE.tsx\"#" \
  "$OUT/web/tsconfig.json"

# --- deploy: product-specific (a hook the config defines) --------------------
product_prepare_deploy "$OUT"

# --- repo furniture ----------------------------------------------------------
cp "$COMMON/LICENSE" "$OUT/LICENSE"
cp "$ROOT/scripts/products/$PRODUCT/README.md" "$OUT/README.md"
mkdir -p "$OUT/.github/workflows"
cp "$COMMON/ci.yml" "$OUT/.github/workflows/ci.yml"

# --- guard: no suite, no sibling product, no deleted-web-area references ------
fail() { echo "export-product: leak — $*" >&2; exit 1; }
[ -e "$OUT/suite" ] && fail "suite/ present"
for d in "$OUT"/products/*/; do
  [ "${d%/}" = "$OUT/$PRODUCT_CRATE_DIR" ] || fail "sibling product ${d#"$OUT"/}"
done
for d in "${PRODUCT_WEB_DELETE[@]}"; do
  hits="$(grep -rIlE "src/$d[\"/]" "$OUT/web/src" 2>/dev/null || true)"
  [ -z "$hits" ] || fail "web references removed src/$d: $hits"
done

echo "exported $PRODUCT ($PRODUCT_REPO) to $OUT"
