# ADR 0014 — Per-tenant DKIM signing keys

**Status:** accepted · 2026-07

**Decision:** DKIM signing keys become **per-tenant, per-domain**, generated
in-process when a domain is verified (ADR 0012), stored in `ficina-store`, and
resolved at sign time from the outbound message's `From` domain. The existing
single deployment key (`FICINA_SMTP_DKIM_*` file/env) stays in place as a
**zero-regression fallback**: a domain with no stored key signs exactly as
today. This is the near-term, unblocked step toward the DNS wizard (ADR 0013);
CNAME indirection later turns rotation no-touch.

**Why now / why this shape.** Ficina Cloud hosts many domains; each must sign
with its own key (`d=` aligned to the sender's domain for DMARC), and the
operator — not the customer — should own key rotation. The `KeyStore` seam in
`ficina-auth-mail` was built for exactly this: it is addressed by
`(domain, selector)` and its doc already frames rotation as "publish a new
selector, sign with it, retire the old — no code change." We add a
store-backed resolver behind that seam; the signer's crypto is untouched.

**Key algorithm: Ed25519 (RFC 8463), generated in-process.** In-process RSA
key *generation* is not available to us — the pure-Rust `rsa` crate carries
RUSTSEC-2023-0071 (forbidden by our crypto rule) and `ring` cannot generate
RSA; shelling to OpenSSL would break "Rust below the waterline." This is the
same constraint that made ADR 0008 choose EdDSA for ID tokens. Ed25519 DKIM is
verified by Gmail, Outlook, and Proton and passes DMARC. The trade-off:
some older receivers and scoring tools ignore Ed25519 and thus see "no DKIM."
Mitigation, additive and not in this slice: an operator may still provide an
**RSA** key for a domain through the existing file keystore (the fallback), and
an RSA-key *import* path (OpenSSL-generated offline, stored) is a recorded
follow-up. We do not generate RSA in-process.

**Storage of private key material.** The PKCS#8 DER private key is stored in a
new tenant-scoped `dkim_keys` table — the same "private key in the system of
record" pattern as the OIDC `signing_keys` table (ADR 0008), relying on
database-at-rest encryption and access control. In memory it is held in
`Zeroizing` buffers and **never logged** (the `KeyStore` types already enforce
this). The public key is stored alongside so the DNS record can be shown
without re-deriving it.

**Signing path.** `sign_outbound` changes from "sign as the one configured
`(domain, selector)`" to: derive the `From` domain → look up that domain's
**active** stored key (its selector + material) → sign `d=<from-domain>;
s=<selector>`. If no stored key exists, fall back to the configured file key
(behaviour byte-identical to today for the current single-tenant deployment).
A signing failure still sends the message unsigned and logs — mail flow is
never lost to DKIM (unchanged).

**Rotation = selector rollover.** One key per `(tenant, domain)` is `active`
(used for signing). Rotating generates a new selector's key, marks it active,
and leaves the previous selector's public record valid for verification until
the customer's DNS has propagated, after which it is removed. Surfaced in the
Domains page as the record to publish + a rotate action.

**Contracts / compatibility.** Additive: a new `dkim_keys` table and new
`/admin/domains/*` + `/control/domains/*` sub-actions (get record, rotate). No
existing route or the file-key env changes. The deployment DKIM env remains
supported and documented as the single-tenant / fallback path.

**Rejected — keep the single deployment key and require operators to manage
per-domain keys by hand (files).** It does not scale to Cloud, puts rotation on
the customer, and defeats the "operator owns deliverability" promise. The file
keystore is retained only as the fallback and the RSA escape hatch.

**Rejected — encrypt each DKIM private key under a per-deployment KEK now.** A
real improvement, but it is a broader secrets-management decision that should
cover the OIDC signing keys too (which today store DER the same way); doing it
for DKIM alone is inconsistent. Tracked with the vault-backed `KeyStore` the
`ficina-auth-mail` doc already anticipates, not bolted on here.
