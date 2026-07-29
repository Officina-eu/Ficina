# Multi-tenant trust boundary — deferred hardening

A security sweep of the tenant-admin console + AI inference layer
returned **no critical or high findings**: tenant scoping on every
store query is airtight, API keys never leave the server, the
`require_admin` gate is enforced on every `/admin/*` route, SMTP
list fan-out is loop-free and same-tenant, and no secret or message
body reaches a log.

It surfaced two **medium** findings that share one root cause: the
**multi-tenant trust boundary is not built yet**. Today Ficina ships
as a single-tenant, self-hosted sovereign deployment — the tenant
admin *is* the box operator, and there is one global mail domain
(`FICINA_SMTP_LOCAL_DOMAINS`). Both findings are only exploitable once
we host multiple mutually-distrusting tenants on shared
infrastructure — the **multi-tenant control plane** (a later phase).
They are recorded here so they close *before* that boundary is
crossed, not after.

A naive patch now would be wrong on both counts (see each below), so
each fix is deferred to the control-plane work with its intended shape
written down.

## Must-fix before hosting mutually-distrusting tenants

1. **Bind tenants to the domains they own.** `create_user`,
   `add_alias`, and `set_group_address` validate an address only as
   "contains `@`" — there is no tenant⇄domain mapping in the store. In
   a shared deployment a tenant-A admin could assign a user/alias/list
   address on tenant B's domain (any not-yet-provisioned address on a
   local domain), capturing inbound mail for it. Canonical-user
   precedence and the ambiguity-refusal in `account_by_email` blunt the
   attack against B's *existing* addresses, but role/future addresses
   can be squatted.

   *Why not now:* the fix is a tenant→authorized-domains table plus a
   domain-verification flow (DNS TXT proof), which is a control-plane
   feature, not a patch. Single-tenant deployments own their whole
   domain, so there is no boundary to cross today.

   *Intended fix:* a `tenant_domains` mapping (verified via DNS), and
   reject any address write whose domain the acting tenant does not own.

2. **Constrain AI backend egress (SSRF).** A tenant admin sets an AI
   provider `baseUrl`; the server then makes outbound HTTP to it
   (`/admin/ai/test` as an unsaved probe, and `/ai/improve` in use).
   There is no IP-range validation, so in a shared deployment an admin
   could point it at the cloud metadata endpoint
   (`169.254.169.254`), `localhost`, or a co-tenant/internal service
   and use the ok/error/timing signal as a blind SSRF / port scanner.
   `require_admin` is not sufficient mitigation when the admin is a
   *customer*, not the infra operator.

   *Why not now:* the obvious guard — block loopback/private ranges —
   is **wrong for this product**: self-hosted Ollama on
   `http://localhost:11434` (and models on the private LAN) is a
   first-class, documented path. A correct guard is therefore
   *deployment-mode aware*: permissive for self-host, default-deny
   private/link-local egress for hosted tenants. That policy switch is
   a control-plane concept.

   *Intended fix:* an egress policy keyed on deployment mode — for
   hosted tenants, resolve the host and reject loopback, link-local
   (incl. `169.254.169.254`), private, and ULA ranges, connecting to
   the pinned resolved IP to defeat DNS rebinding, and require `https`
   for non-loopback hosts (so the key + draft are never sent in
   cleartext); for self-host, allow private/loopback as today.

   *Note:* the migration comment in `0011_ai_providers.sql` predates
   the editable-endpoint feature and wrongly states the outbound URL is
   "not a user-editable field, so it cannot be an SSRF vector." It is
   user-editable. The comment is left in place because the migration is
   already applied on deployments (editing it would break the sqlx
   checksum); this note is the correction of record.

## Already hardened in the same sweep

Not deferred — fixed immediately, since none of these depend on the
multi-tenant boundary:

- **Response-size cap on AI backend replies** — a hostile/broken
  backend can no longer force an unbounded in-memory buffer
  (`ficina-ai` streams with a 4 MiB ceiling).
- **Per-route body limit on `/ai/improve`** — the draft cap (64 KiB)
  is enforced before buffering, not the large blob-upload ceiling.
- **`set_default_ai_provider` no longer silently disables AI** — a
  stale/foreign id rolls back and returns `NotFound` instead of leaving
  the tenant with no default.
- **`create_user` rolls back on partial provisioning** — a failure
  setting the password or inbox deletes the user row, so no
  half-created account can linger.
