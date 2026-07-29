# ADR 0012 — Multi-tenant control plane: platform operator, domain ownership, tenant lifecycle

**Status:** accepted · 2026-07

**Decision:** Shared multi-tenancy (Ficina Cloud) gets a dedicated control
plane, `ficina-control`, as its own service — the fourth long-running HTTP
role alongside `ficina-smtp`, `ficina-jmap`, and `ficina-imap`. It
introduces a **platform-operator** principal distinct from a tenant admin,
a **tenant lifecycle** (create / suspend / resume / delete), and a
**tenant→domain ownership** binding that is the security spine of shared
hosting. The data layer is already multi-tenant (row-scoped by an opaque
`tenant_id`, isolation enforced by the `for_tenant`/`for_account` handles —
ADR 0006, and the isolation the store's door guarantees); this ADR builds
the plane that *governs* tenants, not their data isolation, which stands.

## Principals and surfaces

Three tiers, each on its own surface; a token never crosses tiers:

| Principal | Surface | Scope |
|---|---|---|
| user | `/jmap/*`, `/mail` | own mailbox in own tenant |
| tenant admin (`users.is_admin`) | `/admin/*` | own tenant only |
| **platform operator** (`users.is_platform_admin`) | **`/control/*`** | all tenants, control operations only |

- A **platform operator is a user** — in a reserved **system tenant**
  (`_platform`, created once) — carrying a new global flag
  `users.is_platform_admin`. This reuses the entire identity stack
  (argon2id credentials, opaque revocable access tokens, OIDC, TOTP — ADR
  0008) rather than inventing a second credential authority.
- **The operator token is not a skeleton key.** It authenticates through
  the same path and resolves to `(system_tenant, operator_user)`. It grants
  access **only** to `/control/*` cross-tenant *control* operations (list /
  create / suspend / delete tenants, assign / verify domains, set quotas).
  It does **not** grant read access to any tenant's mail, files, or
  `/admin/*` surface — those stay bound to the token's own tenant by the
  store's door. Governing a tenant and reading inside it are different
  capabilities; only the first is cross-tenant. Impersonating a tenant admin
  for support is an explicit, audited, separate future capability, not a
  side effect of the operator flag.
- Bootstrapped by `identityctl bootstrap-operator <email>` (creates the
  `_platform` tenant on first call, then the operator user + flag) — the
  same deliberately-non-public, CLI-only origin as the first tenant admin
  (ADR 0008 lineage). There is no HTTP signup for operators.

## Why a separate `ficina-control` service (not `/control/*` on jmap)

The architecture contract already names `ficina-control` as the control
plane's home (ARCHITECTURE.md layer 3; the crate exists as an empty
Phase-2 placeholder). Three reasons make the separation worth its cost (a
new container + Caddy route + healthcheck):

1. **Open-core boundary.** The product doc lists "which control-plane
   components stay proprietary" as an open commercial decision. A separate
   crate/service is the seam along which that boundary can later be drawn
   without disentangling it from the AGPL mail core. Folding `/control/*`
   into `ficina-jmap` would weld the two together.
2. **Blast radius.** The control plane provisions and deletes whole
   tenants; keeping it a distinct process with its own credentials and its
   own restart/rollout envelope means a control-plane bug cannot take mail
   delivery down with it, and the operator surface can be firewalled to an
   ops network independently of the public JMAP surface.
3. **Contract clarity.** `/control/*` is an operator contract with a
   different audience, lifecycle, and compatibility story than the tenant
   `/admin/*` and `/jmap/*` contracts.

The service depends on `ficina-store` and `ficina-identity` exactly as
`ficina-jmap` does; cross-service communication stays API/store-mediated,
never shared in-process state (ARCHITECTURE.md standing rule).

## Tenant→domain ownership — the security spine

Today a domain is a single flat, deployment-wide allowlist
(`FICINA_SMTP_LOCAL_DOMAINS`), and any tenant admin can assign an address
in any local domain — in shared hosting, a mail-hijack path (recorded in
`docs/design/multi-tenant-trust-boundary.md`). Fix:

- New deployment-global **`domains`** table: `domain` (PK, globally unique,
  lowercased) → `tenant_id`, with `verified_at` and a `verify_token`. One
  domain belongs to exactly one tenant; the PK makes a second claim
  impossible.
- **Verification by DNS TXT:** publish `_ficina-verify.<domain> = <token>`;
  a verify call resolves it (reusing the hickory resolver from the Security
  & trust checks) and stamps `verified_at`. Assigning addresses requires a
  **verified** domain.
- **Ownership enforced on address writes:** `create_user`, `add_alias`,
  `set_group_address` reject an address whose domain is not a verified
  domain owned by the acting tenant.
- **Inbound "is this local" derives from the table**, not only the env
  list: a domain is local iff it has a verified `domains` row. The env
  `FICINA_SMTP_LOCAL_DOMAINS` remains as the dev / single-tenant fallback
  when the table is empty.

### Rollout — enforcement is flagged, default off

Ownership enforcement is behavior that would break an existing
single-tenant deployment (its addresses have no `domains` row yet), so per
the implement skill it ships behind `FICINA_ENFORCE_DOMAIN_OWNERSHIP`
(default **false**). With the flag off, behavior is exactly as today
(single global local-domain set). Going multi-tenant is then: register +
verify each tenant's domains, then flip the flag — a config change, and its
own rollback. The domains table, its management API, and the backfill are
shippable immediately; only the *refusal* is gated.

## Tenant lifecycle

- `tenants` gains **`status`** (`active` | `suspended`, default `active`)
  and `created_at` already exists. A **suspended** tenant fails auth closed
  (login denied) and its inbound RCPT is refused with a transient `450`
  (mail is retried by senders, not bounced, so a billing lapse is
  recoverable). Suspension is reversible and touches no data.
- **Create:** `POST /control/tenants` runs the existing
  `bootstrap_admin` provisioning (tenant + first admin + inbox) behind the
  operator gate — the operation SSH + CLI does today, made an audited API.
- **Delete** is destructive and GDPR-relevant: DB rows cascade from
  `tenants` (`ON DELETE CASCADE` is already on every tenant table); Garage
  blob GC for the deleted tenant is a **recorded follow-up** (the bytes are
  content-addressed and dedup-scoped per tenant, so orphaned objects are a
  storage-cost bug, not a leak). Delete requires an explicit confirmation
  token in the request and is always audited.

## Contracts and compatibility

- `/control/*` is a **new** operator contract — additive, no existing
  surface changes.
- `users.is_platform_admin`, `tenants.status`, and the `domains` table are
  **additive** schema (expand-only; each column has a safe default,
  existing rows stay valid). No contract breaks; no destructive migration.
- `/admin/*`, `/jmap/*`, and the session document are unchanged for
  existing tenants.
- User- and operator-facing strings are externalized (i18n) from the first
  line.

## Rejected alternatives

- **Rejected — a global super-admin flag that unlocks tenant management
  inside the existing `/admin` console** (one console, no new service). It
  conflates "runs the platform" with "runs one tenant" on a single surface,
  welds the control plane to the AGPL core across the open-core boundary,
  and gives a control-plane bug the same blast radius as tenant mail.
  Cleaner to separate now than to extract under load later.
- **Rejected — a separate operator credential authority** (operators not
  modeled as users). It would duplicate the argon2/token/OIDC/2FA
  machinery `ficina-identity` already provides and tested; a reserved
  system tenant reuses all of it while keeping operators off every tenant's
  data by the same door that isolates tenants from each other.
- **Rejected — database-per-tenant or schema-per-tenant isolation** (a
  common SaaS default) instead of row-scoping. Out of scope for this ADR —
  it is settled by ADR 0006 and the store's design; row-scoping with the
  `for_tenant` door is what the tenant-isolation tests already prove, and
  this ADR does not disturb it.
- **Rejected — enforcing domain ownership unconditionally now.** It would
  break the live single-tenant deployment on deploy. The flagged rollout
  gets the mechanism in and lets the switch be thrown deliberately.

## Consequences

- `ficina-control` becomes a deployed service: a new compose service, a
  Caddy route (`/control/*` on the ops surface), a healthcheck, and its own
  image built from the same workspace. Operator auth reuses `/auth/token` /
  OIDC against `ficina-identity`.
- The deferred findings in `docs/design/multi-tenant-trust-boundary.md`
  close here (domain ownership) and in the follow-up egress-policy slice
  (AI-endpoint SSRF, made deployment-mode-aware).
- The open-core boundary decision (product doc §15) now has a concrete
  seam to be drawn along; this ADR does not itself close it.
