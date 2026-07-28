# Ficina — single-server production deployment

This runs the **complete first-class mail path** on one server and one
database: **receive** mail, **read** it from any mail app, **send**, and
**log in** securely. It is a real deployment, not a demo.

**Honest scope:** the day-one inbox is a **mail app (Thunderbird, Apple
Mail, a phone)** over IMAP — there is **no browser webmail yet** (that is a
separate Phase-2 build). Everything needed for a mail app to work is here.

## What runs

| Service | Purpose | Ports |
|---|---|---|
| `ficina-smtp` | receive (25) + authenticated send (587/465) + outbound + spam/trust stack | 25, 587, 465 |
| `ficina-imap` | read mail from a mail app | 993 (IMAPS), 143 (STARTTLS), 995 (POP3S) |
| `ficina-jmap` | native API **and** the OpenID Connect login provider (behind Caddy) | internal 8080 |
| `caddy` | automatic Let's Encrypt HTTPS for the login/API origin | 80, 443 |
| `postgres` | system of record | internal |
| `rspamd` | spam scoring at receive time | internal |

Message bodies are stored on a shared on-disk volume all three Ficina
services mount (single-node; multi-node would swap in Garage/S3).

## Prerequisites

- A Linux server with Docker + the compose plugin, ports 25/80/443/465/587/
  993/143/995 reachable from the internet.
- DNS for your domain (`DOMAIN`, e.g. `mail.example.com`):
  - `A`/`AAAA` `mail.example.com` → this server,
  - `MX` for the mail domain → `mail.example.com`,
  - `PTR` (reverse DNS) for the server IP → `mail.example.com`,
  - `A` `mta-sts.mail.example.com` → this server (for the MTA-STS policy),
  - SPF/DMARC as you prefer; **DKIM is generated below**.

## Setup

```sh
cd deploy/production
cp .env.example .env
# Edit .env: DOMAIN, FICINA_SMTP_LOCAL_DOMAINS, ACME_EMAIL.
./generate-secrets.sh          # fills POSTGRES_PASSWORD with fresh randomness
./generate-dkim.sh             # writes dkim/dkim.key and PRINTS the DNS record
# → add the printed TXT record at fic._domainkey.<your-domain>, then continue.

docker compose up -d --build   # build the images and start the stack
```

Watch it come up and become healthy:

```sh
docker compose ps              # every service should read "healthy"
docker compose logs -f caddy   # first boot: Caddy obtains the Let's Encrypt cert
```

Create your first admin mailbox (password read from the environment, never
the command line):

```sh
docker compose exec -e FICINA_ADMIN_PASSWORD='a-strong-password' \
  ficina-jmap identityctl bootstrap-admin your-org you@your-domain.com
```

## Connect a mail app (the day-one inbox)

In Thunderbird / Apple Mail / your phone:

- **Incoming (IMAP):** server `mail.example.com`, port **993**, SSL/TLS,
  username = your full email, password = the one you just set.
- **Outgoing (SMTP):** server `mail.example.com`, port **465** (SSL/TLS) or
  **587** (STARTTLS), same username/password.

Send yourself a message and reply to it to confirm the full loop.

## The login provider (OIDC)

Once up, the OpenID Connect endpoints are live at `https://<DOMAIN>`:
`/.well-known/openid-configuration`, `/oauth/authorize`, `/oauth/token`,
`/oauth/userinfo`, `/oauth/jwks`. Register a first-party app (e.g. a future
webmail) with:

```sh
docker compose exec ficina-jmap \
  identityctl register-client web "Ficina Web" https://<DOMAIN>/callback
```

## ⚠️ TLS certificate model — one decision to confirm

Caddy **automatically obtains and renews** a Let's Encrypt certificate for
`<DOMAIN>` and terminates HTTPS for the web/login origin — that part is
fully automatic. The **mail services (SMTP/IMAP) reuse that same
certificate** by reading it from Caddy's storage volume (mounted read-only
at `/certs`). This keeps one cert for the whole server. Two consequences to
be aware of:

1. **First-boot ordering.** The mail cert only exists after Caddy has
   obtained it. On the very first `up`, bring Caddy up first (or just wait a
   minute and `docker compose restart ficina-smtp ficina-imap` once
   `docker compose logs caddy` shows the certificate obtained).
2. **Renewal reload.** Caddy renews automatically; the SMTP/IMAP services
   pick up a renewed cert on restart. A periodic
   `docker compose restart ficina-smtp ficina-imap` (e.g. a monthly cron) or
   an automatic reload-on-change is a small follow-up.

**This is the one deployment choice worth confirming.** The alternative is a
dedicated ACME sidecar (certbot/lego) writing a stable cert path that every
service — including Caddy — reads. Say the word and I'll switch to that; it
removes the storage-path coupling at the cost of one more container. Either
way, **the TLS path can only be fully verified against a real public domain**
(Let's Encrypt won't issue for a private/local name).

## Local test mode (no public domain)

To validate the stack on a laptop without Let's Encrypt, set in `.env`:

```sh
FICINA_SMTP_ALLOW_SELF_SIGNED=true
FICINA_IMAP_ALLOW_SELF_SIGNED=true
FICINA_SMTP_TLS_CERT=
FICINA_SMTP_TLS_KEY=
FICINA_IMAP_TLS_CERT=
FICINA_IMAP_TLS_KEY=
```

and create `docker-compose.override.yml` making Caddy use its internal CA:

```yaml
services:
  caddy:
    command: ["caddy", "reverse-proxy", "--from", "localhost", "--to", "ficina-jmap:8080"]
```

The services then present self-signed certs (mail apps will warn) — fine for
a smoke test, never for production.

## How to notice problems, and how to turn it off

- **Notice:** `docker compose ps` shows per-service health; `docker compose
  logs -f <service>` streams structured logs (no secrets are ever logged).
  A detected token replay or a failed revoke logs a `warn` a monitor can
  alert on.
- **Turn off outbound sending** (kill-switch): set
  `FICINA_SMTP_OUTBOUND_ENABLED=false` in `.env` and
  `docker compose up -d ficina-smtp`.
- **Stop everything:** `docker compose down` (data volumes persist);
  `docker compose down -v` also deletes the data (irreversible).

## What is deliberately NOT here

Browser **webmail** (Phase 2), and the **chat/meet/docs** engines
(Synapse/LiveKit/Collabora — separate Phase-2 products, in the dev compose
one level up). The multi-tenant / self-service hardening items in
`docs/design/security-audit-followups.md` are also deferred; none affect a
single-owner mailbox.
