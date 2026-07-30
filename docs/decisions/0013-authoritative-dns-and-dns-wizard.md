# ADR 0013 — Authoritative DNS and the registrar-universal DNS wizard

**Status:** proposed · deferred (not before paying customers / a dedicated
operator) · 2026-07

**Context.** Domain onboarding today is manual-and-universal: Ficina shows
the records to publish (`_ficina-verify` TXT, MX, SPF, DKIM, DMARC, mta-sts)
and *reads* DNS to verify them (ADR 0012 + the Security & trust checks). It
works at every registrar because it only reads. The "just works" onboarding
vision — one click, no copy-paste, keys that rotate themselves — needs Ficina
to also *write* DNS. This ADR records **how**, and why it waits.

**Decision (direction, not yet built).** The universal automation path is
**nameserver delegation to Ficina-run authoritative DNS** — NOT per-registrar
APIs and NOT Domain Connect.

- **Why not registrar APIs / Domain Connect:** they are not universal.
  Domain Connect is implemented by only a handful of registrars (GoDaddy,
  IONOS, …) — not Namecheap, not most. A registrar-API approach would leave
  the majority of customers on the manual path and isn't sovereign. The one
  mechanism *every* registrar supports is changing the domain's `NS` records.
- **The universal path:** the customer repoints their domain's nameservers to
  Ficina's authoritative DNS. Ficina then serves the whole zone — MX, SPF,
  DKIM (with self-service rotation), DMARC, autodiscover, mta-sts — regardless
  of registrar. Sovereign (we run it, not a third party) and universal (NS
  change works anywhere). Trade-off, stated honestly: delegation moves *all*
  of the customer's DNS to us, including their website records, so it is
  **opt-in**; manual + verify remains the always-available floor.

**Engine choice — pinned upstream, configured not patched (ADR 0003).**
Recommendation: **Knot DNS** (CZ.NIC), with PowerDNS as the fallback.

- **Knot DNS** — authoritative-only, small, fast, permissively licensed
  (GPLv3 but run as a separate process behind our API, so no linking
  concern), first-class dynamic updates and online DNSSEC signing, a clean
  control socket (`knotc`)/`libknot` for zone edits. Authoritative-only fits
  exactly what we need (we are not a recursive resolver). Chosen for the small
  attack surface and native automated DNSSEC.
- **PowerDNS Authoritative** — fallback: an HTTP API and SQL/database backends
  make programmatic zone management ergonomic, but it is a larger surface and
  the API/DB coupling is more to operate.
- **NSD** — rejected as the primary: rock-solid and minimal, but static
  zone-file oriented with weaker dynamic-update/DNSSEC-automation ergonomics,
  so per-tenant record churn and key rotation are more awkward.

**DNSSEC is in scope from day one** when we serve zones — an authoritative
DNS that we operate should sign. Knot's automated key management is a reason
it leads.

**The wizard, in tiers (universality is the hard constraint):**
1. **DKIM via CNAME indirection** — the customer publishes a static CNAME
   (`<selector>._domainkey.<domain>` → a Ficina zone) once, and Ficina rotates
   the underlying key forever. Universal (all registrars support CNAME) but
   **depends on this ADR** — the CNAME target only resolves once Ficina serves
   authoritative DNS. It is the *flip* that turns the per-tenant DKIM keys
   (ADR 0014, built now) into no-touch rotation.
2. **Full NS-delegation autopilot** — Ficina serves the whole zone; onboarding
   becomes "change your nameservers to these two," and every record is managed
   and monitored automatically.
3. **Domain Connect** — optional convenience for the registrars that support
   it; never the strategy.

**Why deferred.** Authoritative DNS is high-stakes production infrastructure
(a zone outage takes a customer's mail *and* website down), it blocks nothing
today (manual + verify is universal and works), and per-tenant DKIM keys (ADR
0014) lay the groundwork regardless. It should land when there are paying
customers to justify it and, ideally, a dedicated operator to own it — not
before. This ADR exists so the direction is settled and the engine chosen when
that day comes; relitigate only with new facts (CLAUDE.md).

**Rejected — a third-party managed DNS (Cloudflare/Route 53) via API for
Ficina's own zone.** It would make DKIM-CNAME work without running a DNS
server, and it is registrar-universal for the customer. Rejected as the
strategy because it puts the sovereignty product's DNS on a US hyperscaler —
acceptable as a stop-gap, not as the design. If used tactically before Knot is
stood up, it must be documented as interim.

**Consequences / non-goals for now.** No DNS engine is added to the workspace
or `deploy/` yet. `features.md` records the "just works" onboarding vision
under a later tier. The near-term, unblocked step is ADR 0014 (per-tenant DKIM
keys), which this ADR's tier 1 later completes.
