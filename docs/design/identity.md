# ficina-identity v1 (design note)

Identity is the authority every other surface trusts. Until now auth was
a **deliberately interim** seam: `ficina-store::auth` hashed passwords
with argon2 and minted opaque bearer tokens for JMAP; `ficina-smtp` had a
plaintext `StaticAuthenticator`; IMAP/POP3 called `Store::verify_login`.
Each carried a comment pointing here. This milestone delivers the real
thing and **deletes the interim**: a new crate `core/ficina-identity`
becomes the single credential authority behind SMTP AUTH, IMAP/POP3
LOGIN, and JMAP bearer, and Ficina gains an OpenID Connect provider so a
customer's other SaaS can one day trust Ficina as its IdP — the M365
lock-in killer.

This is the last security-critical foundation before real users and real
internet mail. Two things must be exactly right: **scoping** (no identity
operation can read, authenticate-as, or enumerate across a tenant or
account boundary) and **credential handling** (every secret compared in
constant time; no secret in a log, error, or `Debug`).

## Dependency shape (no cycle)

```
ficina-smtp  ┐
ficina-imap  ┼─→ ficina-identity ─→ ficina-store ─→ Postgres
ficina-jmap  ┘
```

`ficina-identity` depends on `ficina-store`; the three protocol crates
depend on `ficina-identity` for authentication and on `ficina-store` for
data. The store never depends on identity, so `account_by_email` /
`for_account` / `for_tenant` — the tenant doors every surface already
consumes — stay in the store. Identity owns **policy and protocol**;
the store owns **persistence and the tenancy-by-construction doors**.
The store's identity tables are reached only through those doors
(`TenantStore` for provisioning a known tenant's users/aliases/groups/
credentials/2FA; `AccountStore` where a user is known), except the
handful of **pre-tenant lookups** that must resolve an unforgeable secret
*to* a tenant before any tenant is known — `account_by_email`,
`resolve_access_token`, `resolve_refresh_token`, `consume_auth_code`,
`oauth_client`. These are keyed on an email or a SHA-256 hash and return
the single owning `(tenant, user)` or nothing; they are the only
methods that may cross tenants, exactly as `account_by_email` and
`resolve_token` already do, and each returns `None` on ambiguity rather
than guessing.

## Credentials & the pinned precondition

Passwords are hashed with **argon2id** (tuned params below, documented as
a contract because rehash-on-verify depends on them). **Every secret
comparison is constant-time:**

- **Passwords:** `argon2` verification is internally constant-time over
  the derived key. The user-existence timing oracle is closed with a
  **dummy verify**: an unknown username still runs one argon2
  verification against a fixed decoy hash, so *wrong password* and *no
  such user* are timing-indistinguishable. This is the LOW finding the
  M3 TLS audit pinned to M9 (`docs/design/tls-and-submission.md`,
  `security-audit-followups.md`); it is closed here and proven by a
  timing test, not asserted.
- **Tokens** (access, refresh, OAuth codes) are opaque high-entropy
  random strings, stored only as their **SHA-256 hash**; lookup is by
  hash (preimage resistance is the constant-time-equivalent posture —
  the presented secret is never compared byte-wise against a stored
  plaintext, only its irreversible hash against an indexed hash).
- **Recovery codes** are stored as SHA-256 hashes and verified with an
  **explicit constant-time compare** (`ring::constant_time::
  verify_slices_are_equal`) against each of the user's unused codes, so
  neither a match nor its position leaks through timing.
- **TOTP** codes are compared constant-time the same way.

The **`subtle`** crate (`ConstantTimeEq`) provides the primitive — the
purpose-built, auditable constant-time comparison. (`ring`'s own
`verify_slices_are_equal` is deprecated as "no side-channel promises," so we
do not use it.) Secrets are wrapped in a redacting `Secret` newtype, are
`zeroize`d where held, and never enter `tracing`.

**argon2id parameters (contract):** `m = 19456 KiB (19 MiB)`, `t = 2`,
`p = 1` — the OWASP-recommended argon2id baseline, overridable per
deployment via `FICINA_IDENTITY_ARGON2_*` for higher-memory hardware.
Stored hashes are self-describing (PHC string), so raising params is
backward-compatible: an old hash still verifies and is transparently
rehashed on next successful login.

## The identity model

`tenants → users → { aliases, credentials, TOTP, recovery codes }`, plus
`groups` (named membership sets within a tenant). A **user** is the
canonical account (`AccountStore`'s `(tenant, user)`); an **alias** is an
additional inbound address routing to a user. `account_by_email` resolves
a canonical address first, then an alias, returning `(tenant, user)` only
when the address maps unambiguously to exactly one user across the
deployment — inbound routing must never guess. Local delivery therefore
files alias-addressed mail into the right account with no change to its
call site. Address uniqueness (a canonical email or an alias) is enforced
globally so routing stays deterministic.

**Admin bootstrap** is a CLI/provisioning path (`identityctl
bootstrap-admin`), never a public endpoint: it creates a tenant and its
first user with a password read from stdin or the environment (never
`argv`, which leaks to the process table). Self-service signup and an
admin HTTP surface are out of scope (a Phase-2 product concern); Phase 1
provisions from the CLI.

Groups exist as a first-class model (create, add/remove member, list) and
are tenant-scoped and isolation-tested, but **group-based authorization**
(shared mailboxes, send-as) is not wired to any decision yet — it lands
with the sender-authorization model (below). Shipping the model now keeps
the schema stable for it.

## Sessions & tokens

- **Access token** — opaque, 32 bytes of `ring::SystemRandom`, URL-safe
  base64; SHA-256 hashed at rest; short-lived (default 1 h). Presented as
  `Authorization: Bearer` to JMAP and resolved to `(tenant, user,
  scope)`.
- **Refresh token** — opaque, hashed at rest, longer-lived (default 30
  d), bound to `(user, client, scope)`; **rotated on use** (the old
  refresh token is revoked and a new one issued; reuse of a rotated token
  revokes the chain — replay defense).
- **Revocation is real:** every token row carries `revoked_at`; a logout
  / `POST /oauth/revoke` sets it and the very next `resolve_*`
  rejects. Access-token TTL bounds the window where a not-yet-expired
  access token outlives a revoked refresh token; both are checked on
  every use.

### Rejected — JWT (self-validating) access tokens

Tempting because the resource server verifies them with the JWKS and no
DB round-trip. **Rejected:** a signed JWT is valid until it expires and
**cannot be revoked** (scope (f) requires that logout actually
invalidates). Opaque access tokens with a server-side hash store give
true revocation and let us keep tokens short without a refresh storm.
The **ID token** *is* a JWT (OIDC Core mandates it) — but it is an
identity assertion consumed once at login, not a bearer capability, so
its un-revocability is correct. Access = opaque + revocable; ID = signed
+ single-shot.

## OIDC / OAuth 2.0 provider

Ficina-as-IdP. Endpoints (issuer = `FICINA_IDENTITY_ISSUER`):

- `GET /.well-known/openid-configuration` — discovery (RFC 8414 / OIDC
  Discovery): `issuer`, `authorization_endpoint`, `token_endpoint`,
  `userinfo_endpoint`, `jwks_uri`, `response_types_supported=["code"]`,
  `grant_types_supported=["authorization_code","refresh_token"]`,
  `code_challenge_methods_supported=["S256"]`,
  `id_token_signing_alg_values_supported=["EdDSA"]`,
  `scopes_supported`, `claims_supported`, `subject_types_supported=
  ["public"]`.
- `GET /oauth/jwks` — JWKS: the current and previous **Ed25519** public
  keys as `{"kty":"OKP","crv":"Ed25519","alg":"EdDSA","kid":…}` (RFC
  8037), so rotation is a publish-both-then-retire.
- `POST /oauth/authorize` — **authorization-code + PKCE** (RFC 6749 §4.1,
  RFC 7636). `S256` challenge **required** (no `plain`, no
  challenge-less code — PKCE-downgrade closed). Validates `client_id`,
  exact `redirect_uri` match, `response_type=code`, `scope`, `state`,
  `nonce`. The resource-owner login (`username`/`password`, then TOTP if
  enabled) is **submitted** here: the first-party web app renders the login
  page and POSTs the credentials plus the OAuth parameters; on success a
  single-use code (hashed, ~60 s) is issued and the browser is redirected
  (303) with `code` + `state`. A tenant-scoped client is refused for a user
  of another tenant; a `NULL`-tenant client is deployment-wide. A
  browser-`GET` render of a consent/login page for a *third-party* RP is a
  later addition (the login UI is Phase-2 webmail); Phase 1 exposes the
  credential-submitting POST.
- `POST /oauth/token` — `authorization_code` (with `code_verifier`,
  verified `S256` against the stored challenge) and `refresh_token`
  grants → `{access_token, token_type:"Bearer", expires_in,
  refresh_token, id_token, scope}`. Codes are single-use (a replay is
  refused and revokes any token minted from that code).
- `GET /oauth/userinfo` — bearer-authenticated; returns the `sub`,
  `email`, and profile claims permitted by the token's scope.

**Scopes & claims (contract, minimal):** `openid` (required; `sub`),
`email` (`email`, `email_verified`), `profile` (`name`, `preferred_
username`). `sub` is the opaque, stable `UserId` — never the email
(email can change; `sub` must not). Additive-only forever.

**Signing & rotation** — ID tokens are signed **EdDSA (Ed25519)** via
`ed25519-dalek`, already in the tree. Keys live in a deployment-global
`signing_keys` table; the newest non-retired key signs, all non-retired
public keys are published in the JWKS with a `kid`, and rotation retires
old keys after a grace window. **Rejected — RS256:** it is the most
widely accepted RP algorithm, but generating/validating RSA keys in pure
Rust means either the `rsa` crate (RUSTSEC-2023-0071, forbidden by our
crypto rule) or a C dependency (breaks Rust-below-the-waterline). EdDSA
is RFC 8037, supported by modern OIDC RP libraries, and keeps us on the
audited `ring`/`dalek` stack. Recorded as an ADR because it is a lasting
contract.

## Protocol integration — the interim is deleted

- **JMAP** — `state::authenticate` resolves a real access token via
  identity; `/auth/token` (interim password grant) is **replaced** by the
  OAuth `authorization_code` flow. A thin **`POST /auth/token` password
  grant is retained only for first-party programmatic clients** (the raw
  JMAP client in the exit-gate) and documented as such — it issues the
  same opaque access token, through identity, with the same constant-time
  path; it is not `password` OAuth grant on the public `/oauth/token`.
- **SMTP** — `StaticAuthenticator` is deleted; the submission AUTH
  PLAIN/LOGIN path verifies through identity's constant-time password
  check. The `Authenticator` trait indirection is removed; `ficina-smtp`
  depends on `ficina-identity` directly (the trait existed only to defer
  this crate — its reason to exist is gone).
- **IMAP/POP3** — `Store::verify_login` is replaced by identity's
  password check; `for_account` is still how the session reaches data.

**Brute-force discipline on token endpoints.** SMTP and IMAP already cap
failed AUTH per connection. The OAuth `token`/`authorize` and the
programmatic `/auth/token` add a **per-(client,username) rolling
failure counter with exponential backoff** (a `429` with `Retry-After`
after N failures in the window), chosen over hard lockout because
lockout is a **denial-of-service lever** an attacker can pull against a
known username; backoff slows brute force without letting a third party
lock a victim out. Recorded in the note as the deliberate choice.

### 2FA on legacy protocols — the app-password seam

TOTP is interactive; SMTP/IMAP/POP3 AUTH is a single password exchange
with nowhere to prompt for a code. Rather than silently accept a
password-only login for a 2FA account — which would give the user a false
sense of protection and let a phished password read their mail over IMAP —
**the legacy protocols fail closed**: `authenticate_legacy` refuses a
TOTP-enabled account (returning the same indistinguishable failure as a
wrong password — no oracle) and the user must authenticate via the OIDC
flow. A non-2FA account authenticates normally. The same method adds a
**per-username backoff across connections** on top of the per-connection
failure caps (a correct-password 2FA refusal is not counted as a failure,
so a legitimate 2FA user is never locked out by trying their password).

**App-specific passwords / `XOAUTH2`** are the sanctioned cut seam that
re-opens legacy clients to 2FA users: a per-client random password that
carries no interactive step but stays revocable. OAuth-capable desktop
clients (Thunderbird supports `XOAUTH2`) already work today via the OIDC
flow; the founder's Thunderbird in the exit gate uses that. The follow-up
is app passwords + `XOAUTH2` SASL on submission so a 2FA user can also use
a non-OAuth client.

## Tenancy

Every identity write is scoped to a tenant through `TenantStore`; every
pre-tenant lookup returns the single owning `(tenant, user)` or nothing
and can never surface another tenant's row. The isolation suite probes
every identity operation across **both** boundaries: tenant A cannot
enumerate, authenticate-as, reset, or read tenant B's users, aliases,
groups, tokens, TOTP, or recovery codes; and within one tenant, account A
cannot reach account B's. A wrong id/email/token is a clean empty/`401`,
never data and never a `500`.

## Failure semantics & enumeration hardening

- Login, password reset, and OIDC errors are **uniform**: `invalid
  credentials` never distinguishes wrong-password from no-such-user (the
  dummy-hash makes it true in timing too). The **inbound `550 5.1.1` at
  RCPT still enumerates deliverable addresses** — that is the conscious
  local-delivery tradeoff (`docs/design/local-delivery.md`), a separate
  surface from the credential endpoints hardened here.
- OAuth errors follow RFC 6749 §5.2 exactly (`invalid_request`,
  `invalid_grant`, `invalid_client`, …) without leaking which check
  failed beyond the spec's categories.

## Out of scope (recorded, deferred)

- **App-specific passwords + `XOAUTH2`** on submission — the sanctioned
  cut seam above; interim is account-password-on-legacy.
- **Sender authorization** (bind submission `MAIL FROM` to the
  authenticated identity / send-as) — still deferred (M3 audit item #2):
  it needs the group/alias permission model this milestone *ships the
  data for* but does not wire to a decision. Named again so it is not
  lost.
- **Confidential OAuth clients / client-credentials & device grants**,
  dynamic client registration (RFC 7591), front/back-channel logout,
  and PAR — the web app is a first-party **public** client (PKCE); these
  are additive later.
- **WebAuthn / passkeys**, SCIM provisioning, and an admin HTTP console —
  Phase 2+.
- **QR image rendering** for TOTP enrollment — we emit the `otpauth://`
  provisioning URI (the actual secret); rendering it as a QR is the web
  layer's job.
