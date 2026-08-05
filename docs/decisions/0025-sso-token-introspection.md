# ADR 0025 — SSO for standalone products via token introspection

Status: accepted. The first concrete step of ADR 0019's "platform-as-
services" end-state: how a genuinely independent product (its own repo,
its own app, its own backend) shares the one workspace login without
linking the platform's source or reaching into its database.

## Context

The workspace is one login across many products (Mail, and next Drive,
Docs…). Until now every product-facing API has been `alo-jmap`, which
links `alo-identity` as a **library** and resolves bearer tokens
in-process. That works while everything is one binary in one tree.

A standalone product breaks that assumption. Drive is its own service in
its own repo; it must authenticate the same users without:

- copying `alo-identity`'s source (duplication — ADR 0019 rejects it), or
- linking the whole platform crate (pulls in the mail-centric store, and
  couples Drive's build to the monorepo), or
- sharing the identity database directly (couples Drive to another
  service's schema and secrets).

`alo-identity` is already an OIDC provider, but its `/oauth/userinfo`
returns only the end-user's `sub` + email — deliberately **not the
tenant**, which is an app/authorization concern, not an OIDC identity
claim. Every Drive read and write is tenant-scoped (Law 1), so the tenant
is exactly what a resource server must learn from a token.

## Decision

Add an **RFC 7662 token-introspection endpoint** to `alo-identity`:
`POST /oauth/introspect`. A resource server (Drive, and every future
standalone product) presents an opaque access token and receives the
principal behind it — `{ active, sub, tenant, scope, username }` — or
`{ active: false }` for an unknown/expired/revoked token.

This is the SSO seam for the services model: products depend on the
identity **API**, not its source or its database. A product validates a
request by calling introspection over the private network; nothing else
of the platform is needed for auth.

### Guards (a validity oracle is a real risk)

- **Off by default.** Disabled (404) unless `ALO_IDENTITY_INTROSPECT_SECRET`
  is configured. It is never accidentally public.
- **Resource-server credential required.** Every call presents that secret
  as a bearer, compared **constant-time** (`subtle`); wrong or missing ⇒
  401. Without this, `/oauth/introspect` — which shares the public
  `/oauth/*` path space — would be a token-validity oracle (RFC 7662 §2.1).
- **No error oracle.** An invalid token is a normal `200 {active:false}`,
  never a 4xx that distinguishes "malformed" from "expired" from "unknown".
- **Revocation-honest.** Introspection reuses `resolve_access_token`, so a
  revoked or expired token reports inactive immediately.

### Rejected alternatives

- **Put the tenant in `userinfo`.** Smaller, but semantically wrong:
  `userinfo` describes the end user to their own client; multi-tenancy is
  a resource-server concern. Introspection is the RFC-blessed seam for
  resource servers, and keeps the end-user surface clean.
- **Drive links `alo-identity` (versioned library).** Real independence
  but coupled (ADR 0019's middle row): Drive's build and DB would follow
  the platform. Introspection is the decoupled option, and the whole point
  of a standalone product.

## Consequences

- Standalone products authenticate with a single network call and need no
  platform source. Drive is unblocked; Docs and the rest inherit the seam.
- The endpoint is additive and default-off — no existing behavior changes,
  and mail's in-process auth is untouched.
- When identity itself becomes a separately-deployed service (ADR 0019),
  this endpoint is already the contract other services depend on.
