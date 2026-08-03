# alo — architecture contract

This file is the design contract. Code that moves it must update it in
the same PR (CLAUDE.md law of the map). Rationale for every major
choice lives in `docs/decisions/` — this file states *what is*, ADRs
state *why*.

## The five layers

1. **Clients** — alo web app (PWA → Tauri desktop → mobile shells);
   Outlook/phones via compat adapters (EAS, later MAPI — year two);
   any standard IMAP/DAV/Matrix client.
2. **Gateway & identity** — single entry point; `alo-identity`
   (built) is the credential authority and OIDC/OAuth2 IdP —
   argon2id credentials, authorization-code + PKCE, opaque revocable
   access tokens, EdDSA ID tokens, and TOTP 2FA (SAML later). Every
   token resolves to a `(tenant, user)` here, and SMTP/IMAP/JMAP
   authenticate through it, so tenant enforcement is anchored HERE
   before any service trusts a request. See ADR 0008.
3. **Core services** —
   built: `alo-smtp`, `alo-store`, `alo-jmap`, `alo-imap`,
   `alo-dav`; integrated engines behind our APIs: Synapse (chat,
   one instance per tenant), LiveKit (meet), Collabora via WOPI
   endpoints we serve (docs). `alo-jmap` also serves the
   **tenant-admin console** API (`/admin/*`): users & mailboxes,
   groups & distribution lists, live deliverability checks, and AI
   provider config — every route gated on an admin claim resolved at
   the gateway (layer 2) and re-checked against `users.is_admin`.
4. **AI layer** — `alo-ai`. Built: the **model-agnostic inference
   API** (ADR 0011) — one wire contract, OpenAI-compatible Chat
   Completions, so the backend is *configured, never bundled*
   (per-tenant: local Ollama, self-hosted model, or a hosted
   provider). Still to come: the event-bus indexer over all stores,
   the per-tenant semantic index, and the MCP server — which sit
   BELOW services so one query spans mail/chat/files.
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
- Monorepo, layered by ADR 0019: `platform/` (the shared kernel —
  store, identity, auth-mail, sieve, ai) → `products/<product>/` (the
  Mail product lives in `products/mail/`: smtp, smtp-client, imap, jmap)
  → `suite/` (the workplace umbrella — control plane, integration).
  Plus `web/` `migrate/` `deploy/` `docs/`. Dependency direction is
  one-way: suite → products → platform; a product never depends on the
  suite or on another product. Rust below the waterline, TypeScript
  above. Nothing else.
