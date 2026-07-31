# Deployment readiness — "one server that does real mail"

**Question this answers:** what stands between today's code and a single
server where you can **send** mail, **receive** mail, **read it in an
inbox**, and **log in securely** — as a real product, not a demo.

**Honest status (2026-07-28):** the *engine-room* code is strong and
verified (SMTP, the message store, IMAP/POP3, JMAP, Sieve filtering, and
the identity/login system all pass their tests and audits). But the code
is a set of **libraries and one runnable service**, not yet a packaged,
deployable system. Three of the four things you want are **not currently
runnable as a server**, and there is no trusted-TLS or webmail story yet.
None of this is bad news about the code — it is the wiring-for-deployment
that Phase 1 always left until the code was proven. It is proven now.

A note on how to read the effort sizes: **S** ≈ up to a day, **M** ≈ a few
days, **L** ≈ a week or more. These are honest engineering estimates for
doing it to the same standard as the rest, not rushed.

---

## Goal 1 — Receive mail

**What exists:** the SMTP receiver (`alo-smtp`) is a real, running
service with a Docker image. It accepts internet mail, runs the full trust
stack (SPF/DKIM/DMARC + spam scoring), and — when connected to the database
— files the message into the right mailbox with the user's filters applied.
This part is genuinely done.

**What's missing (all configuration, no new code):**

| Gap | Why it matters | Effort |
|---|---|---|
| The deployment config doesn't connect the receiver to the database | Without it, received mail has nowhere to land (it would sit in a spool, not an inbox) | **S** |
| The hosted-domain list isn't set for the real domain | The server needs to know which addresses are "local" (yours) vs. relay | **S** |
| MX + PTR DNS must point at the server | So other mail servers know to deliver here (you've already done PTR + MX) | done |

## Goal 2 — Send mail

**What exists:** the submission service (authenticated send on ports
587/465) and the outbound delivery queue both exist in `alo-smtp`, with
DKIM signing built in. Sending is wired; it's just switched **off by
default** for safety.

**What's missing:**

| Gap | Why it matters | Effort |
|---|---|---|
| Outbound sending is disabled by default | A deliberate safety catch so a misconfigured server can't become a spam relay; must be turned on intentionally | **S** |
| A DKIM signing key must be generated and its public half added to DNS | So Gmail/Outlook trust your mail and it lands in inboxes, not spam | **S** |
| A real (Let's Encrypt) TLS certificate | Sending/receiving over trusted TLS instead of a self-signed cert | shared with Goal 4 |

## Goal 3 — Read mail in an inbox  ← the biggest gap

**What exists:** the IMAP/POP3 server *logic* is written, tested, and
audited — it correctly serves mailboxes to any mail app (Thunderbird, Apple
Mail, a phone). **But there is no program that starts it.** It is a library
with a `serve()` function and no entrypoint, no Docker image, and no entry
in the deployment config. The same is true of the JMAP API (the native
protocol a future web inbox would use).

**What's missing:**

| Gap | Why it matters | Effort |
|---|---|---|
| An IMAP server **program** (entrypoint + Docker image + config) | Without it there is literally no way to open your inbox from a mail app | **S–M** |
| A JMAP/API server **program** (entrypoint + Docker image + config) | The native API and the login provider run inside this; needed for logins and a future web inbox | **S–M** |
| **Webmail** — a browser inbox | "Read mail in a browser" is a Phase-2 product (a whole web app). It does **not** exist yet | **L** (deferred) |

**Honest near-term answer:** a *readable inbox* on day one means **a desktop
/ phone mail app (e.g. Thunderbird) over IMAP** — which needs the small IMAP
program above. A **browser** inbox (webmail) is a much larger Phase-2 build
and is not part of "one working server." I will not pretend otherwise.

## Goal 4 — Log in securely

**What exists:** the full identity system — argon2id passwords, 2FA, an
OpenID Connect / OAuth login provider — is built, tested, and audited (Stage
1). The admin-account CLI works. The login provider is served *inside* the
JMAP program.

**What's missing:**

| Gap | Why it matters | Effort |
|---|---|---|
| The JMAP program that hosts the login provider (same as Goal 3) | The login endpoints only run when that server runs | shared with Goal 3 |
| A trusted TLS certificate + a small reverse proxy in front | Logins send a password, so the front door must be real HTTPS (Let's Encrypt), not self-signed; the proxy routes 443 → login/API, and terminates TLS | **M** |
| The first admin mailbox created (`disan@namel3ss.com`) | Your actual account — one CLI command once the above runs | **S** |

---

## The cross-cutting gaps (apply to all four)

| Gap | Why it matters | Effort |
|---|---|---|
| **Real secrets** | The sample config ships with obvious dev passwords/tokens (`…dev-only`); production needs freshly generated secrets, stored safely | **S** |
| **Trusted TLS + reverse proxy** | One place (e.g. Caddy) to get/renew Let's Encrypt certs and route the mail + web ports; also serves the MTA-STS policy the code already generates | **M** |
| **Refresh the deployment config** | It still references a credentials mechanism that was removed, and doesn't include the IMAP/JMAP services | **S** |
| **Health checks for the new services** | So the stack reports honestly whether receive/read/login are actually up | **S** |

---

## What Stage 3 will build (the plan)

To turn the above into "a server that really works," Stage 3 produces, to
production standard:

1. **Two small server programs** — one for IMAP/POP3, one for the JMAP API +
   login provider — each with a Docker image and environment-based config,
   mirroring how the SMTP service is already built.
2. **A production deployment file** that runs the full path — receive
   (SMTP) + read (IMAP) + login/API (JMAP) — all connected to one database,
   with real health checks.
3. **A reverse proxy (Caddy) with automatic Let's Encrypt TLS**, routing the
   web port and terminating TLS, and serving MTA-STS.
4. **Real secrets handling** (generated, not the dev defaults) and outbound
   sending correctly enabled with DKIM.

**Explicitly still deferred after Stage 3** (so nothing is oversold): a
**browser webmail inbox** (Phase 2) — the day-one inbox is a desktop/phone
mail app over IMAP; the chat/meet/docs engines (Synapse/LiveKit/Collabora),
which are separate Phase-2 products; and the multi-tenant/self-service
hardening items recorded in `docs/design/security-audit-followups.md`.

**Rough total for Stage 3:** a handful of days of focused work — mostly the
two small server programs, the TLS/proxy layer, and honest config, not new
engine code. The hard, security-critical parts (the actual mail engine and
the login system) are already built and verified.
