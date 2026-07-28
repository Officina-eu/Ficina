# ADR 0008 — Identity provider: opaque revocable access tokens, EdDSA ID tokens

**Status:** accepted · 2026-07

**Decision:** `ficina-identity` is Ficina's credential authority and an
**OpenID Connect / OAuth 2.0 provider**. Its token model is:

- **Access tokens are opaque** — high-entropy random, stored only as a
  SHA-256 hash, resolved to `(tenant, user, scope)` server-side, and
  **revocable** (a `revoked_at` column checked on every use). Not JWTs.
- **Refresh tokens are opaque, hashed, and rotated on use** (reuse of a
  rotated token revokes the chain).
- **ID tokens are JWTs signed with EdDSA (Ed25519)**, published via a
  JWKS with `kid` rotation (RFC 8037).
- The only OAuth flow is **authorization-code + PKCE `S256`** (public
  first-party client); `plain` PKCE and challenge-less codes are refused.

Identity depends on `ficina-store`; the store keeps the tenant-door
(`for_tenant`/`for_account`/`account_by_email`) and gains identity tables,
so there is no dependency cycle. The interim auth (`ficina-store::auth`,
`ficina-smtp::StaticAuthenticator`, the JMAP `/auth/token` argon2 path) is
**deleted**, not left behind.

**Why opaque + revocable access tokens:** scope requires that logout
actually invalidates a session. A self-validating JWT access token cannot
be revoked before it expires without exactly the server-side lookup that
makes it no longer self-validating — so a JWT access token buys a DB
round-trip saved at the cost of the revocation guarantee we must keep. An
opaque token hashed at rest gives real revocation, lets access tokens stay
short (1 h) with silent refresh, and reuses the exact pattern the interim
`api_tokens` table already proved. The **ID token** is a JWT because OIDC
Core mandates it and because it is the one artifact an external Relying
Party must verify *without* calling us — but it is a single-shot identity
assertion, not a bearer capability, so its un-revocability is correct.

**Why EdDSA, not RS256:** RS256 is the most universally accepted RP
signing algorithm, and choosing EdDSA is a real interop cost (an RP with
a hardcoded RS256 assumption cannot verify our ID tokens). We choose EdDSA
anyway because generating and handling RSA keys in pure Rust requires
either the `rsa` crate — carrying **RUSTSEC-2023-0071** (a Marvin timing
vulnerability), which our crypto rule forbids — or an OpenSSL/C dependency
that breaks "Rust below the waterline." `ring` does not generate RSA keys.
Ed25519 (via the audited `ed25519-dalek`, already in the tree for DKIM) is
RFC 8037, constant-time by construction, and supported by modern OIDC RP
libraries (`openid-client`, `jose`, Keycloak, Authelia, …). The algorithm
is advertised honestly in discovery
(`id_token_signing_alg_values_supported: ["EdDSA"]`) so an RP negotiates
or rejects at integration time, never silently. If a future customer's RP
is RS256-only, an RS256 key type is additive to the JWKS and the signer
the day we can source RSA keys without the forbidden crate — the JWKS and
`kid` machinery are built for exactly that.

**Rejected — a stateless JWT-everything design (access + ID + refresh all
JWTs, no token table).** It removes the token store entirely and scales
without shared state, which is why many small IdPs reach for it. Rejected
for the revocation reason above and because it would make "log out this
device" and "revoke a leaked token" impossible to honor truthfully — a
promise we would rather not print than not keep.

**Rejected — reusing the interim `/auth/token` password grant as the
public auth surface.** It is a username/password POST that mints a token;
extending it to the web app and third parties would standardize a
password-in-POST anti-pattern and an RP would send user passwords to
Ficina. The OAuth authorization-code flow keeps the password on Ficina's
own login page only. The password grant is **retained solely as a
first-party programmatic convenience** (the raw JMAP exit-gate client),
issuing the same opaque token through the same constant-time path, and is
documented as non-public.

**Consequences:** the discovery document, JWKS, the `/oauth/*` endpoints,
and the scope/claims set are a **public contract** from merge — additive
changes only (CLAUDE.md "contracts outlive code"). `sub` is the opaque
stable `UserId`, never the email. The argon2id parameters are a documented
contract (a stored PHC hash is self-describing, so raising them is
backward-compatible with transparent rehash-on-login). App-specific
passwords and `XOAUTH2` on submission — how a 2FA user drives a legacy
IMAP/SMTP client — are the sanctioned cut seam; the interim is
account-password-on-legacy, TOTP enforced on the browser flow. See
`docs/design/identity.md`.
