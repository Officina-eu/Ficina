# ADR 0004 — Garage for object storage; MinIO ruled out

**Status:** accepted · 2026-07

**Decision:** Garage (Deuxfleurs, AGPL-3.0, Rust, France) is Ficina's
S3-compatible blob store.

**Why:** MinIO's community edition was wound down through 2025 and the
repository archived read-only in April 2026 — the exact single-vendor
rug our integration doctrine exists to survive. Garage is actively
maintained, community-governed, designed for self-hosted multi-node
deployments, EU-made, and in our language.

**Rejected:** MinIO (dead upstream); SeaweedFS/Ceph (heavier
operational profiles than our tenant sizes need — revisit at scale).

**Consequences:** replication topology configured across two EU
locations per the DR targets; admin-API provisioning of per-tenant
buckets from ficina-control; this ADR is the standing example cited
when anyone proposes depending on a single-vendor "open" project.
