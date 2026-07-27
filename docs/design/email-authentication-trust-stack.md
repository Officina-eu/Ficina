# Email authentication trust stack (design note)

Phase 1 milestone M4: email authentication (SPF, DKIM, DMARC, ARC),
transport-security policy (MTA-STS, TLS-RPT), and spam scoring (Rspamd)
for `ficina-smtp`. Delivered as a new crate `ficina-auth-mail` (the
component ARCHITECTURE.md designates for exactly this), which
`ficina-smtp` calls at DATA time (inbound verdicts) and at submission
(DKIM signing).

## Surface

`ficina-auth-mail` exposes, per RFC:

- **SPF** (RFC 7208): `check_host(ip, helo, mail_from)` → result
  (`pass`/`fail`/`softfail`/`neutral`/`none`/`temperror`/`permerror`),
  full mechanism set (`all`, `a`, `mx`, `ip4`, `ip6`, `include`,
  `exists`, `ptr` [discouraged, §5.5]), `redirect`/`exp` modifiers,
  macro expansion (§7), and the hard limits: ≤10 DNS-querying
  mechanisms (§4.6.4) and ≤2 void lookups → `permerror`.
- **DKIM** (RFC 6376): verify (multiple signatures, `relaxed`/`simple`
  header+body canonicalization, `l=` body-length, `x=` expiry, `t=`)
  and sign (RSA-2048 and Ed25519/RFC 8463). Keys addressed by
  `(domain, selector)` behind a `KeyStore` trait.
- **DMARC** (RFC 7489): policy discovery via the organizational domain
  (public-suffix list), SPF+DKIM alignment (relaxed/strict),
  disposition (`none`/`quarantine`/`reject`), and aggregate-report XML
  (Appendix C). Report *delivery* reuses the M2 queue (a later step).
- **ARC** (RFC 8617): chain validation + sealing.
- **MTA-STS** (RFC 8461) policy text + **TLS-RPT** (RFC 8460) report
  JSON for our own domains.
- **Authentication-Results** (RFC 8601): the one builder that renders
  every verdict — see Contracts.

`ficina-smtp` integration: at DATA (before `250`) it runs SPF (on the
MAIL FROM domain + connecting IP), verifies DKIM signatures, evaluates
DMARC, consults Rspamd, stamps `Received-SPF` and
`Authentication-Results`, and applies disposition. On submission it
DKIM-signs the outbound message.

## Contract — `Authentication-Results` (RFC 8601)

**From this milestone on, `Authentication-Results` is a public
contract.** Every verdict — `spf=`, `dkim=` (one per signature),
`dmarc=`, `arc=`, and the Rspamd spam result — is recorded there and
nowhere else; downstream consumers (store, JMAP, web UI) parse THIS
header, not our internal types. One `authserv-id` per deployment (the
configured hostname), consistent RFC 8601 formatting, folded safely.
Changes to the rendered format are additive only.

`Received-SPF` (RFC 7208 §9.1) is also stamped, for operators and
legacy tooling, but the authoritative machine-readable record is
`Authentication-Results`.

## Security posture (auditor will check)

- **DKIM private keys**: the file-based `KeyStore` refuses a key file
  that is group- or world-readable (Unix mode check at load), never
  logs or formats key material into errors, and holds key bytes in
  `zeroize`-ing buffers. The key path is always configured, never
  defaulted into the repo tree.
- **DNS is hostile input**: every lookup goes through one `resolver`
  module with timeouts and record-count/length caps; TXT contents are
  parsed defensively (bounded, no trust in length or charset).
- **No panics on malformed input**: a malformed signature, policy, or
  DNS record yields a *fail verdict*, never a crash — these bytes come
  from the open internet. Enforced by the workspace `unwrap`/`panic`
  deny-lints plus fuzz-style unit tests.

## Rejected alternatives

- **KeyStore = concrete file struct (no trait).** Rejected: M9/ops
  will hold DKIM keys in a vault (per the product doc's
  secrets-in-a-vault rule), and rotation means keys are looked up per
  `(domain, selector)` at sign time. A trait now (file impl today,
  vault impl later) keeps that swap from touching the signer. Cost is
  one layer of indirection — worth it for a key-management seam.
- **Rspamd fail-open on unreachable.** Rejected as the default: if the
  scanner is down, failing open silently disables spam/phishing
  filtering — the worst time to accept everything. Default is
  fail-closed (`451` tempfail, sender retries); per-tenant fail-open
  is a later opt-in for operators who prefer availability over
  filtering. Deliverability of our own outbound is unaffected (this is
  inbound scoring).
- **Integrating an existing auth library (e.g. mail-auth).** Rejected:
  ARCHITECTURE.md designates `ficina-auth-mail` as built by us, and
  the trust stack's exact behavior (lookup-limit hardness, fail-closed
  policy, the Authentication-Results contract, key rotation) is
  policy we must own and evolve, not inherit. Crypto primitives
  (`rsa`, `ed25519-dalek`, `sha2`), DNS (`hickory`), and the
  public-suffix list (`psl`) are integrated — those are commodity and
  not our differentiation.

## Scope cut for this session (recorded)

M4 is large. Built to full depth in priority order: SPF, DKIM
(verify+sign), Authentication-Results, DMARC, Rspamd, MTA-STS. The
sanctioned cut seams, left as unchecked ROADMAP sub-items if the
session runs long:

- **ARC sealing** (d) — needed for mailing-list forwarding, not for
  first-hop receive/submit; deferred with its own follow-up.
- **TLS-RPT report generation** (e) — the MTA-STS policy is served;
  the TLS-RPT *reporting* side is deferred.

Anything cut is recorded in ROADMAP.md as an unchecked sub-item, never
shipped as a stub.
