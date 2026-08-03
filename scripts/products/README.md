# Product export configs (ADR 0019)

Each public product repo (`alomails`, later `alodocs`, `alocalendar`, …) is
**generated** from this monorepo by a shared engine — `scripts/export-product.sh`
(out) and `scripts/backport-product.sh` (in), driven by the reusable
`publish-product.yml` / `backport-product.yml` workflows. The engines are never
copied; a product is a small config plus two thin caller workflows.

## Adding a product

1. **Config** — `scripts/products/<product>.sh`. Set `PRODUCT_REPO`,
   `PRODUCT_CRATE_DIR` (`products/<product>`), `PRODUCT_WEB_SURFACE`, the
   `PRODUCT_WEB_DELETE`/`PRODUCT_WEB_EXTRA` areas to drop, and a
   `product_prepare_deploy` hook. Copy `mail.sh` as a starting point.
2. **README** — `scripts/products/<product>/README.md` (the product repo's front
   page). `LICENSE` and CI come from `common/`, shared by all products.
3. **Web surface** — `web/src/product/<surface>.tsx` (the product's module set;
   added when the product's web is built).
4. **Publish workflow** — `.github/workflows/publish-<product>.yml`: a ~12-line
   caller of `publish-product.yml` passing `product`, `target_repo`, and the
   App secrets.
5. **Back-port workflow** — `.github/workflows/backport-<product>.yml`: a caller
   of `backport-product.yml`.
6. **GitHub App** — install it on the product repo (Contents + Workflows: write)
   and add its `<PRODUCT>_APP_ID` + `<PRODUCT>_APP_PRIVATE_KEY` repo secrets.

That's it — the export/back-port logic is inherited, not duplicated.

## Layout

```
scripts/
  export-product.sh          shared export engine
  backport-product.sh        shared back-port engine
  products/
    <product>.sh             per-product config (repo, crates, web, deploy hook)
    <product>/README.md      per-product repo README
    common/
      LICENSE                AGPL-3.0, shared
      ci.yml                 product-repo CI, shared
      gen-manifest.py        narrows the workspace to platform + the product
```
