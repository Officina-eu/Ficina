# ADR 0006 — Message store on Postgres, blobs in Garage via object_store

**Status:** accepted · 2026-07

**Decision:** `ficina-store` keeps the system of record in PostgreSQL
(via `sqlx` with compile-checked queries) and message *bytes* in Garage
(S3), reached through the `object_store` abstraction. The S3/Garage
backend is behind the crate's `garage` cargo feature; the default build
carries only the lean in-memory/local backends so tests and CI do not
compile the cloud client.

**Why sqlx + compile-checked queries:** the store is load-bearing; a
query that drifts from the schema must fail the build, not production.
`sqlx::query!` checks every statement against the schema (a committed
`.sqlx` offline cache lets CI build without a live database).

**Why object_store for blobs:** Garage speaks S3 (ADR 0004). Rather
than hand-roll S3 SigV4, we use `object_store`, the maintained S3
abstraction, which also gives an in-memory backend for fast
deterministic tests — the same `BlobStore` code runs against both.

**Rejected — a `tenant_id` parameter on every store call:** enforcement
would then live in caller discipline; one forgotten argument is a
cross-tenant leak that compiles. We make isolation structural instead
(the `TenantStore` handle), so "forget the tenant" is unrepresentable.
(Full reasoning in `docs/design/message-store.md`.)

**Rejected — Postgres RLS as the primary isolation line:** RLS is
bypassed by table-owner/`BYPASSRLS` roles and `SECURITY DEFINER`
functions and depends on a per-connection GUC a pooled connection can
leak; a policy that fails open on a mis-set GUC is invisible. We keep
enforcement in query text we control and test; RLS may be layered later
as belt-and-braces, never as the only line.

## Transitive advisory acceptances (`.cargo/audit.toml`)

`cargo audit` scans `Cargo.lock`, which retains optional/feature-gated
dependencies even when they are not compiled. Three advisories are
accepted with rationale and tracked for removal:

- **`rsa` — RUSTSEC-2023-0071 (Marvin timing sidechannel).** Not
  compiled: a phantom lock entry from sqlx's optional `sqlx-mysql`
  backend, which we do not enable. `cargo tree -i rsa` is empty. This is
  the crate we deliberately banned from our *compiled* crypto in M4
  (DKIM uses ring); the ban's intent — no vulnerable timing crypto in
  our binary — is fully honored, since `rsa` is never linked.
- **`quick-xml` — RUSTSEC-2026-0194 / -0195 (XML-parse DoS).** Compiled
  only under the `garage` feature, where object_store's S3 client parses
  responses from our own Garage cluster (trusted infra), never
  attacker-controlled XML. Availability-only; not reachable in our
  threat model.

Each ignore is removed the moment upstream ships a fix (sqlx dropping
the unused backend from the lock; object_store bumping quick-xml).

**Consequences:** the store API is unreachable without a tenant context;
blob deletion is deferred to a GC sweep (we ref-count/mark, never delete
on the delivery path); production builds enable `--features garage` for
durable S3 storage.
