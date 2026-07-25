# Ficina — architecture contract

This file is the design contract. Code that moves it must update it in
the same PR (CLAUDE.md law of the map). Rationale for every major
choice lives in `docs/decisions/` — this file states *what is*, ADRs
state *why*.

## The five layers

1. **Clients** — Ficina web app (PWA → Tauri desktop → mobile shells);
   Outlook/phones via compat adapters (EAS, later MAPI — year two);
   any standard IMAP/DAV/Matrix client.
2. **Gateway & identity** — single entry point; `ficina-identity` is
   the OIDC/SAML IdP; 2FA; tenant enforcement happens HERE, before
   any service sees a request.
3. **Core services** —
   built: `ficina-smtp`, `ficina-store`, `ficina-jmap`, `ficina-imap`,
   `ficina-dav`; integrated engines behind our APIs: Synapse (chat,
   one instance per tenant), LiveKit (meet), Collabora via WOPI
   endpoints we serve (docs).
4. **AI layer** — `ficina-ai`: event-bus indexer over all stores,
   per-tenant semantic index, model-agnostic inference API, MCP
   server. Sits BELOW services so one query spans mail/chat/files.
5. **Data** — PostgreSQL (system of record), Garage (S3 blobs),
   vector index (pgvector first). Three boring stores, by design.

## Standing structural rules

- Engines are sealed: pinned upstream containers, configured from
  `deploy/`, spoken to only via their public APIs. No forks.
- All cross-service communication goes through defined APIs or the
  event bus — never shared tables.
- Tenancy is structural: per-tenant DB scoping, per-tenant buckets,
  per-tenant Synapse instance, tenant claim enforced at the gateway
  and re-checked at the store.
- Compat adapters translate at the edge into JMAP; the core never
  learns MAPI/EAS concepts.
- Monorepo: `core/` `web/` `control/` `migrate/` `deploy/` `docs/`.
  Rust below the waterline, TypeScript above. Nothing else.
