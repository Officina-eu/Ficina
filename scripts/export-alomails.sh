#!/usr/bin/env bash
# Export the alomails product repo from the alo-workplace monorepo (ADR 0019).
#
# alomails is the Mail product on its own: the shared platform + the mail
# product + the mail-configured web app + a single-server deploy, under AGPL.
# The export is DETERMINISTIC — the only transforms are removing the suite (the
# control plane and the Docs editor) and defaulting the web to the mail
# product. Everything else is a straight copy, so this never drifts.
#
# Usage: scripts/export-alomails.sh <output-dir>
# Run locally to inspect the result, or by the publish-alomails workflow.
set -euo pipefail

OUT="${1:?usage: export-alomails.sh <output-dir>}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TPL="$ROOT/scripts/alomails"
PYTHON=""
for p in python3 python; do
  if "$p" --version >/dev/null 2>&1; then PYTHON="$p"; break; fi
done
[ -n "$PYTHON" ] || { echo "export-alomails: no working python found" >&2; exit 1; }

rm -rf "$OUT"
mkdir -p "$OUT"

# --- Rust: the shared platform + the mail product + the migrator ---------
cp -r "$ROOT/platform" "$ROOT/products" "$ROOT/migrate" "$ROOT/.sqlx" "$ROOT/.cargo" "$OUT/"
cp "$ROOT/Cargo.lock" "$ROOT/.gitignore" "$ROOT/.gitattributes" "$OUT/"

# The mail-only workspace manifest = the monorepo manifest minus the suite
# member (derived, so shared [workspace.dependencies] never drift out of sync).
grep -vE '"suite/alo-control",|# suite — the workplace umbrella' "$ROOT/Cargo.toml" > "$OUT/Cargo.toml"

# The mail service Dockerfiles must not COPY the (absent) suite layer.
for f in "$OUT"/products/mail/*/Dockerfile; do
  sed -i '/^COPY suite \.\/suite$/d' "$f"
done

# --- deploy: the single-server mail compose only (not the dev compose or the
# Garage/LiveKit configs, which belong to the other suite products) ----------
mkdir -p "$OUT/deploy"
cp -r "$ROOT/deploy/production" "$OUT/deploy/production"
"$PYTHON" "$TPL/strip-control.py" \
  "$OUT/deploy/production/docker-compose.yml" \
  "$OUT/deploy/production/Caddyfile"

# --- web: the mail product surface (ADR 0019 seam) -----------------------
mkdir -p "$OUT/web"
tar cf - -C "$ROOT/web" --exclude=node_modules --exclude=dist --exclude=.turbo . \
  | tar xf - -C "$OUT/web"
# Delete the suite-only source and the workspace surface; default to mail.
# web/scripts is authoring (Docs) codegen only, not part of the build.
rm -rf "$OUT/web/src/control" "$OUT/web/src/authoring" "$OUT/web/src/product/workplace.tsx" \
  "$OUT/web/scripts"
sed -i -E 's#const product = .*#const product = "mail";#' "$OUT/web/vite.config.ts"
sed -i -E 's#"\./src/product/workplace\.tsx"#"./src/product/mail.tsx"#' "$OUT/web/tsconfig.json"

# --- repo furniture: licence, readme, CI ---------------------------------
cp "$TPL/LICENSE" "$OUT/LICENSE"
cp "$TPL/README.md" "$OUT/README.md"
mkdir -p "$OUT/.github/workflows"
cp "$TPL/ci.yml" "$OUT/.github/workflows/ci.yml"

# --- guard: nothing suite-only may leak into the public repo -------------
if grep -rIlE 'suite/alo-control|src/(control|authoring)|product/workplace' "$OUT" \
     --exclude-dir=.git >/dev/null 2>&1; then
  echo "export-alomails: suite-only reference leaked into the export:" >&2
  grep -rInE 'suite/alo-control|src/(control|authoring)|product/workplace' "$OUT" \
    --exclude-dir=.git >&2 || true
  exit 1
fi

echo "exported alomails to $OUT"
