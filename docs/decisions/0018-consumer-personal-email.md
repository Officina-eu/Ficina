# ADR 0018 — Consumer personal email (self-service addresses on a platform domain)

Status: accepted (scope reversal approved by the product owner; build in slices)

## Context

Section 14 of the product description listed **"consumer/free-tier
email"** as a Non-goal, with the standing rule that such entries are
"revisited only with revenue on the table and a written case — the
default answer stays no." This ADR is that written case.

The product owner has decided to add a **personal email** product line:
an individual can self-register an address such as
`johnsmith@alomails.com` on a platform-operated domain, without an
administrator, tenant, or owned domain. This complements — does not
replace — the B2B offering (organisations bringing their own verified
domains).

Two facts about the existing architecture shape the design:

- **Inbound resolution refuses ambiguity.** `Store::account_by_email`
  matches `lower(email)` across all tenants with `LIMIT 2` and returns
  `None` (mail refused) if two users share an address
  (`core/alo-store/src/store.rs`). On a shared domain, two tenants each
  holding `johnsmith@alomails.com` would break delivery for both. Global
  address uniqueness is therefore mandatory, not optional.
- **Login username is already globally unique.** `credentials_username`
  is a global unique index, and provisioning sets `username = email`
  (`core/alo-identity/src/provision.rs`). This constraint is the natural
  global-uniqueness guard for a personal address — provided user +
  credential are created in one transaction so a lost race leaves no
  dangling user row to make `account_by_email` ambiguous.

## Decision

Build personal email as a **thin surface over the existing tenant
model**, changing no isolation invariant.

1. **One tenant per person.** Each personal signup provisions its own
   single-user tenant. "The tenant is sacred" (Law #1) holds unchanged —
   a personal user is isolated exactly as a company is. No shared
   consumer tenant; per-user ACLs are not introduced.

2. **Platform-operated personal domains, config-driven.** A new
   `ALO_PERSONAL_DOMAINS` (comma-separated, e.g. `alomails.com`) marks
   the domains open to self-service. These are platform-owned (the
   operator sets DNS/MX/DKIM once), never tenant-owned; the per-tenant
   domain-ownership path (ADR 0013/0014, `ALO_ENFORCE_DOMAIN_OWNERSHIP`)
   does not apply to them.

3. **Global address uniqueness via one-transaction provisioning.**
   Provisioning creates tenant + user (`email`) + credential
   (`username = email`) + the standard mailboxes in a single
   transaction. A duplicate address hits the global `credentials_username`
   unique index and rolls the whole transaction back, surfaced as a typed
   `AddressTaken`. No new global-email index is added (the credential
   already is the guard), and no dangling user is ever left behind.

4. **Reserved localparts are refused.** RFC 2142 / role names
   (`postmaster`, `abuse`, `hostmaster`, `admin`, `webmaster`, `noreply`,
   `security`, `root`, `mailer-daemon`, `support`, …) and anything below a
   minimum length cannot be self-claimed. The localpart charset is
   restricted (lowercase `a–z`, digits, `.`, `-`, `_`, not leading/
   trailing/`..`).

5. **Verification-gated activation.** Signup collects the desired
   address, a password, and a recovery email; a code is sent to the
   recovery address and the account is **provisioned only after
   verification**. Unverified signups create no tenant — an abuser cannot
   spawn tenants by hitting the endpoint. Pending signups expire.

6. **Isolated consumer sending reputation.** Consumer outbound uses a
   sending identity (IP / DKIM `d=`) separate from B2B tenants, so a
   consumer abuser cannot damage paying customers' inbox placement. The
   B2B and consumer reputation pools are kept apart at the outbound layer.

## Architecture

- **Provisioning primitive** (`alo-store`/`alo-identity`):
  `provision_personal(domain, localpart, password)` — validates the
  localpart, runs the single transaction above, creates Inbox + Sent +
  Drafts + Junk + Trash + Archive, returns the account or `AddressTaken` /
  `Reserved` / `InvalidAddress`.
- **Inbound** reuses `account_by_email` unchanged; global uniqueness makes
  it resolve each personal address to exactly one account.
- **Signup HTTP surface** (`POST /signup/*`, unauthenticated, rate-
  limited, on the `/signup/*` Caddy-proxied prefix): availability check,
  begin (send code), verify (provision). Never reveals whether a recovery
  address exists beyond what the flow needs.
- **Web**: a public signup page (address availability, password, recovery
  email, verification step), separate from the tenant login.
- **Abuse controls**: per-IP and per-recovery-address rate limits;
  reserved names; verification; the standard spam stack on inbound.

## Consequences

- **Deliverability**: isolating consumer sending protects B2B customers,
  at the cost of a second reputation pool to warm and monitor.
- **Moderation & legal**: public signup brings abuse handling, takedown,
  and consumer-scale GDPR data-subject obligations — a real, ongoing ops
  commitment, accepted deliberately.
- **Positioning**: the B2B pitch previously *sold* the absence of a
  consumer tier; that messaging is revised alongside this ADR.
- **Isolation unchanged**: because every personal user is its own tenant,
  no code path gains cross-user reach; the tenancy tests keep their force.

## Build slices (each: built → tested → deployed → verified)

1. This decision record (ADR + product-doc + `features.md`). ✅
2. Provisioning primitive (`Identity::provision_personal`) + reserved-name/
   charset validation + tenant-isolation and uniqueness tests. ✅
3. Signup HTTP surface (`/signup/available`·`begin`·`verify`) + `pending_signups`
   (migration 0035) + salted-hash codes + attempt cap + IP/recovery rate
   limiting. ✅ Deployed **dormant** (disabled until `ALO_PERSONAL_DOMAINS` is
   set) and wire-verified safely refusing all input.
4. Public web signup page (`/signup`, i18n EN + FR): a three-step flow
   (address + availability → code → password) over the `/signup/*` API, plus
   `GET /signup/domains` so the page and the sign-in link hide themselves when
   signup is off. ✅ Deployed **dormant** and wire-verified (page serves, the
   domains list is empty, no signup link).
5. Isolated consumer sending identity (IP/DKIM `d=`) + `ALO_PERSONAL_DOMAINS`
   + `alomails.com` DNS/MX/DKIM + operator docs — the **go-live** step. ⏳

## Alternatives rejected

- **One shared consumer tenant** — rejected: it moves isolation from the
  tenant boundary to per-user ACLs, contradicting Law #1 as written and
  enlarging the blast radius of any bug.
- **A global unique index on `lower(email)`** — rejected for now: a
  broader schema change than needed; the existing `credentials_username`
  index already enforces global uniqueness when provisioning is
  transactional.
- **Open (unverified) signup** — rejected: a spam/abuse magnet on a shared
  sending domain, the exact reputation risk the trust stack exists to
  prevent.
