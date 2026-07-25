# ADR 0003 — Chat, meet, docs, storage: integrate engines, never build

**Status:** accepted · 2026-07

**Decision:** Synapse (Matrix) for chat — one instance per tenant;
LiveKit for meetings; Collabora Online via WOPI for document editing
(OnlyOffice as fallback); Garage for S3 blob storage. All run as
pinned upstream containers behind our APIs and our UI. Source patches
to engines are forbidden without a new ADR.

**Why:** Each engine is 5–15 years of work with zero differentiation
value for Ficina. Our moat is the mail core + the AI layer that spans
all stores — possible precisely BECAUSE we control the integration
seams, not the engines. Per-tenant Synapse gives GDPR-clean isolation
and simple scaling. Users never see engine UIs: Ficina's frontend is
the product (one design system, one search, one settings).

**Rejected:** building any of them (opportunity-cost suicide);
Element's client UI (breaks the one-product feel); MinIO (see 0004).

**Consequences:** three integration seams per engine (identity in,
events out, provisioning around), all living in ficina-control and
ficina-ai; engines are swappable organs — proven necessary by 0004.
