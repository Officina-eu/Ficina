# Openprovider reseller API — integration notes (for the [S+] domain-selling item)

Groundwork for "sell domains in-product" (features.md → alo Sites [S+]).
We hold an Openprovider membership (EU wholesale registrar, NL). This maps
their REST API (v1beta, swagger at https://docs.openprovider.com/swagger.json,
fetched 2026-08-07) to alo's future flows. **No build before the ADR + the
prerequisites (EU PSP checkout; DNS decision below).**

## Environments & auth

- Live: `https://api.openprovider.eu` · Sandbox (CTE): `https://api.cte.openprovider.eu`
  — all development and loop tests run against the CTE only.
- `POST /v1beta/auth/login` (username + password) → bearer token for
  subsequent calls. Credentials are **production secrets**: server env only,
  never this (public) repo; enable Openprovider's IP whitelist for the prod
  server when live. Verify token TTL + re-login strategy during the build.

## The flows we need → their endpoints

| alo flow | Openprovider endpoints |
|---|---|
| Search & price in the buy box | `POST /v1beta/domains/check` (availability), `GET /v1beta/domains/prices`, `POST /v1beta/domains/suggest-name` (alternatives when taken), `GET /v1beta/tlds` (offered TLD list + requirements) |
| Owner registration data | `POST /v1beta/customers` → a reusable contact handle per tenant (name, address, email — GDPR: this is registrant data the registry requires) |
| Purchase | `POST /v1beta/domains` (register with the customer handle + nameservers) |
| Lifecycle | `.../renew`, `.../restore` (post-expiry grace), `/domains/transfer` + FOA/approve endpoints (bring-your-domain-in), `/domains/{id}` get/update |
| Billing reconciliation | `GET /v1beta/domains/prices` at buy time; Openprovider invoice/transaction endpoints for our cost-side bookkeeping |

## DNS: the useful surprise

Openprovider also hosts DNS zones via API (`/v1beta/dns/zones` CRUD +
records). That gives the domain-selling item **two possible DNS phases**:

1. **Phase 1 (lower barrier):** register the domain AND create its zone at
   Openprovider by API — alo writes the mail/site records into that zone the
   moment the purchase completes. Full "live in minutes" onboarding without
   running our own DNS yet.
2. **Phase 2 (full sovereignty, already in features.md):** alo-run
   authoritative DNS (PowerDNS-class sealed container); bought domains point
   at our nameservers instead. Phase 1 zones migrate over.

The ADR should weigh phase 1 as the shipping shortcut — it collapses the
"alo-run DNS" prerequisite from blocking to eventual.

## Retail posture (already decided in features.md)

Honest flat pricing, no first-year-bait renewals, thin margin by design —
the feature is the onboarding/retention closer, not a profit line.
