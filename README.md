# alo workplace

**The sovereign, AI-native workspace for Europe.**

> **History:** this project was formerly named **Ficina**. The umbrella brand is
> **alo** (lowercase wordmark); the suite is **alo workplace**. See
> [ADR 0016](docs/decisions/0016-rebrand-ficina-to-alo.md) for the rename.

alo workplace is a complete replacement for Microsoft 365, built and hosted in
Europe. It combines mail, calendar, chat, video meetings, file storage, and
document editing into one product, unified by an AI assistant that runs on
European infrastructure and sees across everything — mail, conversations,
meetings, and files — inside a single tenant.

> Everyone else built a cheaper Exchange. alo is the first workspace where
> the mail thinks — open, European, and complete enough to cancel Microsoft.

## Two editions, one codebase

| Edition | How it works |
|---|---|
| **alo Cloud** | Hosted by us on EU infrastructure. Change DNS, migrate, done. |
| **alo Self-Hosted** | The same product on your own hardware or European VPS, for organizations that require full physical control. |

## The modules

| Module | What it does |
|---|---|
| **alo Mail** | Full business email: mailboxes, aliases, shared mailboxes, distribution lists, server-side rules (Sieve), archiving, spam and phishing filtering. |
| **alo Agenda** | Calendars, shared calendars, free/busy, invitations, room and resource booking, out-of-office. |
| **alo Chat** | Channel-first messaging with real threads, reactions, powerful search, and guest access. History is queryable knowledge, not a paywalled archive. |
| **alo Meet** | Video meetings integrated with Agenda and Chat. |
| **alo Drive** | File storage and sharing with permissions, desktop sync, lossless Office-format files. |
| **alo Docs** | Documents, spreadsheets, and presentations edited in the browser, in Microsoft formats. |
| **alo AI** | The unifier: semantic search across all modules, inbox triage, thread and meeting summarization, drafted replies, attachment understanding, and an MCP server for your own AI agents. |
| **alo Migrate** | The M365 exit suite: tenant audit, identity takeover, mail/calendar/file migration with permissions, dual delivery, rollback — complete enough that a 30-person company can cancel M365 the day after migrating. |
| **Admin console** | Tenant management, users and groups, domains, deliverability autopilot, audit logging, GDPR exports, backups. |

## Why this exists

Twenty-five years of Exchange alternatives failed the same ways: ideology
instead of a better experience, broken Outlook and phones, hobbyist-grade
administration, feature-parity chasing, and no sales channel. alo is built
against each failure:

- **Better, not parity** — the server itself is intelligent; AI is the
  architecture, not a bolt-on add-on at €30/user.
- **Users notice nothing** — standard protocols plus Exchange-compatibility
  adapters keep Outlook, Apple Mail, and phones working.
- **One binary to run** — deliverability autopilot (DNS wizard, DKIM rotation,
  DMARC monitoring, blacklist alerts) built in.
- **A bundle against the bundle** — Mail + Agenda + Chat + Meet + Docs + Drive
  in one install, so Teams can't be the hook that keeps you on Microsoft.
- **Trust you can verify** — open source, auditable, GDPR-native, EU-hosted.

## Technology

Rust below the waterline (SMTP, mail authentication, storage, JMAP, IMAP,
CalDAV/CardDAV, identity, AI orchestration, control plane, migration),
TypeScript above it (the entire user-facing product). Best-in-class open
source engines — Matrix/Synapse, LiveKit, Collabora Online, Rspamd,
PostgreSQL, Garage — run as version-pinned containers behind our APIs, never
forked. See [ARCHITECTURE.md](ARCHITECTURE.md).

## Repository layout

```
core/      Rust services: smtp, auth-mail, store, jmap, imap, dav, identity, ai
web/       TypeScript web application: all modules, one design system
control/   Multi-tenant control plane: provisioning, quotas, billing hooks
migrate/   alo Migrate: the M365 exit suite (Graph API based)
deploy/    Container composition, pinned engine versions, infrastructure
docs/      Product, protocol, and operations documentation
```

## Status

**Phase 0 — Foundations.** The current milestone: first SMTP message accepted
on a test domain. Roadmap: mail core → product layer → AI layer → migration
suite → public launch.

## Contributing

Read [CLAUDE.md](CLAUDE.md) for the engineering rules (they apply to humans
too) and [ARCHITECTURE.md](ARCHITECTURE.md) for the system design. Every
change ships end-to-end: implementation, error paths, tests, and docs in the
same PR.

## License

Open source core under AGPL-3.0, with a commercial license available
(dual licensing). Final open-core boundary to be fixed before launch.
