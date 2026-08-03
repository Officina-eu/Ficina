# Mail product (alomails) export config (ADR 0019) — sourced by
# export-product.sh / backport-product.sh. Uses $ROOT and $PYTHON from the
# engine's scope.

# The public product repo (owner/name).
PRODUCT_REPO="aloworld-org/alomails"
# The product's Rust crates under the monorepo.
PRODUCT_CRATE_DIR="products/mail"
# The web product surface (web/src/product/<surface>.tsx) to default to.
PRODUCT_WEB_SURFACE="mail"
# Suite-only / other-product web areas to drop from this product's web build.
PRODUCT_WEB_DELETE=(control authoring)
# Non-source web extras to drop (the Docs equation-symbol codegen).
PRODUCT_WEB_EXTRA=(scripts)

# Deploy: the single-server mail stack, with the control-plane service removed.
# A product without a deploy yet would leave this a no-op.
product_prepare_deploy() {
  local out="$1"
  mkdir -p "$out/deploy"
  cp -r "$ROOT/deploy/production" "$out/deploy/production"
  "$PYTHON" "$ROOT/scripts/products/mail/strip-control.py" \
    "$out/deploy/production/docker-compose.yml" \
    "$out/deploy/production/Caddyfile"
}
