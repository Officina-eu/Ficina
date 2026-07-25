//! # ficina-control — multi-tenant control plane (not yet implemented)
//!
//! Future role (ARCHITECTURE.md, layer 3 seams): tenant provisioning,
//! quotas, billing hooks, and the provisioning APIs around every
//! integrated engine (per-tenant Synapse instances, Garage buckets,
//! LiveKit projects). Owns the three integration seams per engine —
//! identity in, events out, provisioning around (ADR 0003).
//!
//! This crate is intentionally empty at Phase 0: it exists so the
//! workspace compiles whole and the monorepo shape from
//! ARCHITECTURE.md is real from the first commit. First real code
//! arrives with the Phase 2 item "Multi-tenant control plane:
//! provisioning APIs for every engine, billing hooks" (ROADMAP.md).
