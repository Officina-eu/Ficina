# alo

**The sovereign, AI-native workspace for Europe.**

*Working name: alo (from the Latin ofalo, "workshop" — the root of the word "office"). Alternative under consideration: Atelier, pending trademark search.*

---

## 1. What alo is

alo is a complete replacement for Microsoft 365, built and hosted in Europe. It combines mail, calendar, chat, video meetings, file storage, and document editing into one product, unified by an AI assistant that runs on European infrastructure and sees across everything — mail, conversations, meetings, and files — inside a single tenant.

It is offered two ways from the same codebase:

- **alo Cloud** — hosted by us on EU infrastructure. The customer changes DNS, migrates, and is done. Recurring subscription.
- **alo Self-Hosted** — the same product, deployed on the customer's own hardware or European VPS, for organizations that require full physical control. License and support revenue.

## 2. Why it exists

Twenty-five years of Exchange alternatives failed for consistent reasons: they sold ideology instead of a better experience, they broke Outlook and phones, they required hobbyist-grade administration, they chased feature parity with Microsoft's past instead of leapfrogging, and no sales channel was paid to sell them.

alo is built against each of those failures:

- **Better, not parity.** alo is the first mail system where the server itself is intelligent — semantic memory of every thread, automatic triage, drafted replies, cross-channel answers ("what did I miss this week?" drawn from mail, chat, and meetings together). Microsoft bolts Copilot onto 30-year-old plumbing at a premium; alo is built around the model, with data that never leaves the tenant.
- **Users notice nothing.** Standard protocols plus Exchange-compatibility adapters mean Outlook, Apple Mail, and phones keep working. Migration is a weekend, not a retraining program.
- **One binary to run.** Deliverability autopilot (DNS wizard, DKIM rotation, DMARC monitoring, blacklist alerts) built into the product — the number-one reason self-hosted mail fails, solved in-product.
- **A bundle against the bundle.** Mail + Agenda + Chat + Meet + Docs + Drive in one install, so Teams can no longer be the hook that keeps a customer on Microsoft.
- **Channel from day one.** European MSPs and resellers earn recurring margin on alo; the people who currently install Microsoft become the people who install alo.
- **Trust that can be verified.** Open source, auditable, GDPR-native, EU-hosted — sovereignty as a checkable fact rather than a promise.

## 3. The product modules

| Module | What it does |
|---|---|
| **alo Mail** | Full business email: mailboxes, aliases, shared mailboxes, native distribution lists, server-side rules (Sieve), archiving, spam and phishing filtering. |
| **alo Agenda** | Calendars, shared calendars, free/busy, meeting invitations, room and resource booking, out-of-office. |
| **alo Chat** | Slack-model messaging: channel-first, real threads, reactions, powerful search, guest access. History is queryable knowledge, not a paywalled archive. |
| **alo Meet** | Video meetings integrated with Agenda and Chat; meeting links native to the calendar. |
| **alo Drive** | File storage and sharing with permissions, synced to desktop, serving Office-format files losslessly. |
| **alo Docs** | Documents, spreadsheets, and presentations edited in the browser, in Microsoft formats, embedded under alo's interface. |
| **alo AI** | **An agent for every product** (ADR 0034) — a Mail agent, Agenda agent, Tasks agent, Docs agent, Sheet agent, Drive agent, and Chat agent, each scoped to its product's data + actions — with **"Ask alo"** above them as the **top-level agent** — ask it in plain language and it answers *and acts* across products, orchestrating the product agents (the universal workspace command bar). All propose-then-approve (never silent), access-scoped, EU-only inference. Plus attachment understanding (reads incoming .docx/.xlsx), a "where did X go?" onboarding assistant, and an MCP server so customers' own AI agents can work with their workspace. |
| **alo Migrate** | The M365 exit suite — see section 6. |
| **Admin console** | Tenant management, users and groups, domains, deliverability autopilot, audit logging, GDPR exports, backups. |

**The Business modules (ADR 0035)** — alo's second act: the operational backbone SAP and Odoo sell, rebuilt from scratch on alo's own foundations (alo Base's relational core, the tenant-scoped store, Spaces permissions, the agent framework), shipped in waves, **each with its own AI agent on day one**:

| Wave | Module | What it does |
|---|---|---|
| B1 | **alo Billing** | Quotes → invoices → payments with **EU e-invoicing** (EN 16931: Factur-X, XRechnung, Peppol) — the compliance wave every EU SME must ride in 2025–2027. Legal gapless numbering, credit notes, VAT summaries, dunning. |
| B2 | **alo CRM** | Deals that live on real mail threads — pipeline board, activities, quotes from deals. The mail-native advantage no standalone CRM has. |
| B3 | **alo Projects** | Client projects + timesheets over the shipped Tasks module; billable hours become invoice lines in one click. |
| B4 | **alo Finance** | Expenses with AI receipt capture, double-entry ledger, bank-statement import (CAMT/MT940/CSV), AI-assisted reconciliation, P&L / balance sheet / VAT reports, accountant role. |
| B5 | **alo Inventory** | Product catalog, suppliers, purchase and sales orders, multi-location stock with full move history, reorder rules. |
| B6 | **alo HR** | Employee records, leave management, org chart, recruitment-lite, one approvals inbox. Payroll *calculation* is a permanent non-goal — data exports to local providers instead. |

## 4. What we build vs. what we integrate

The rule: **build where we differentiate, integrate the commodity, operate the rest as sealed containers.** Our repositories stay Rust + TypeScript only.

### Built by us (from scratch, Rust)

| Component | Role |
|---|---|
| `alo-smtp` | Mail transfer and submission: receiving (port 25), client submission (587), queueing, routing, retries. |
| `alo-auth-mail` | DKIM signing/verification, SPF, DMARC, ARC, TLS, MTA-STS/DANE. |
| `alo-store` | The message and blob store: mailboxes, threads, flags, attachments, full-text indexing, on PostgreSQL + object storage. |
| `alo-jmap` | JMAP API (RFC 8620/8621) — the native protocol for our own clients. |
| `alo-imap` | IMAP/POP3 compatibility shim over the store for legacy clients. |
| `alo-dav` | CalDAV/CardDAV server: calendars, contacts, invitations (iTIP/iMIP), free/busy. |
| `alo-identity` | Users, groups, OIDC/SAML identity provider, LDAP sync, 2FA. alo can act as the company's SSO. |
| `alo-ai` | Event-bus indexer, per-tenant semantic index, LLM orchestration, triage/summarization/drafting workers, MCP server. |
| `alo-control` | Multi-tenant control plane: provisioning, quotas, billing hooks, monitoring. |
| `alo-migrate` | The full M365 migration suite (Graph API based). |
| Compatibility adapters | Exchange ActiveSync (phones), then MAPI-over-HTTP (native Outlook) — translating at the edge into JMAP calls. Post-launch roadmap. |
| **Web application** | The entire user-facing product in TypeScript: mail, agenda, chat, meet, drive, docs shells, admin console — one design system so five engines feel like one product. |
| **Desktop & mobile shells** | PWA at launch; Tauri desktop app in phase two — the same TypeScript UI inside a thin Rust shell (tray, native notifications, autostart, deep links), followed by an offline-first layer: a local Rust mail cache syncing over JMAP, sharing types and client logic with `alo-core`, so desktop users get instant search and mail that works on the train. Mobile apps reuse the same UI and sync engine. No third language ever enters the codebase — Rust below the waterline, TypeScript above it, on every platform. |

### Integrated open source (pinned upstream containers, never forked)

| Project | Role in alo | Language (irrelevant to users, relevant to ops) | Origin |
|---|---|---|---|
| **Synapse (Matrix)** | Chat engine behind alo Chat. alo ships its own UI; Matrix is the invisible protocol. Swappable later for a Rust homeserver. | Python | UK/EU community |
| **LiveKit** | WebRTC engine behind alo Meet, used via its components SDK under our UI. | Go | Open source |
| **Univer** (Sheets) + **BlockNote** (Docs) | The editor *engines*, embedded as TypeScript **libraries** under alo's own UI (ribbon, chrome, import/export) — not sealed containers. Replaces the Collabora-via-WOPI shell (**ADR 0033**, superseding ADR 0010); real Office files are imported best-effort, not round-tripped. Slides is a native in-house canvas (no open engine covers it). | TypeScript (Apache-2.0 / MPL-2.0) | Open source |
| **Rspamd** | Spam and phishing scoring at SMTP time in phase one; augmented and potentially replaced by `alo-ai` scoring later. | C / Lua | EU community |
| **PostgreSQL** | System of record for all structured data. | C | Open source |
| **Garage** | S3-compatible blob storage: attachments, files, media. (MinIO ruled out — its community edition was archived in 2026.) | Rust | France (Deuxfleurs) |
| **Vector index** (pgvector or Qdrant) | Semantic search index for the AI layer. | C / Rust | Open source (Qdrant: Berlin) |
| **Tauri** | Desktop application shell. | Rust | Open source |

Engines are pulled as version-pinned container images and configured from our `deploy/` directory. Upstream security updates are a version bump; we never carry forks.

### Deliberately not built

Office editors (decades of format compatibility — integrated instead), video/WebRTC internals, spam-scoring engine (initially), payment processing, device management (MDM — referred to EU partners).

### The integration doctrine

Engines are appliances; alo is the building. Every integrated *container* engine is contacted exclusively through its public API — Matrix client/admin APIs and application services for Synapse, SDK/JWT/webhooks for LiveKit, S3 and admin API for Garage. (The editor engines — Univer, BlockNote — are instead embedded TypeScript **libraries** under our UI, not containers; ADR 0033.) Three consequences follow: our code is never derivative of theirs (AGPL obligations stay at "link upstream"); every engine is swappable when an upstream falters, as MinIO's 2026 archival demonstrated; and no API limits exist, because the engines run on our infrastructure — the only rate limits in the system are the per-tenant fair-use limits we impose at our own gateway, which double as pricing-tier boundaries. Engines run unmodified as version-pinned containers; any unavoidable patch goes in a public patches repo and is offered upstream. No engine rebuild is considered before alo has paying customers — after that it is a deliberate investment decision per component, not a reflex.

## 5. Architecture

Five layers:

1. **Clients** — alo web/PWA/desktop/mobile apps; Outlook and phones via compatibility adapters; any standard IMAP/DAV/Matrix client.
2. **Gateway and identity** — single entry point, OIDC/SAML, 2FA, tenant enforcement.
3. **Core services** — Mail core and Agenda (built), Chat and Meet (integrated engines behind our API).
4. **AI layer** — sits *below* the services, indexing all stores through one event bus; this placement is the moat, because Microsoft's separate products cannot share a brain.
5. **Data** — PostgreSQL, S3-compatible object storage, vector index. Three boring stores, which keeps backup, GDPR export, and migration tractable.

One monorepo (`core/`, `web/`, `control/`, `migrate/`, `deploy/`, `docs/`); integrated engines live outside it as pinned dependencies.

**Client strategy.** Where the data lives and what users click on are independent decisions. Sovereignty is a server-side promise: companies that want nothing in the cloud run alo Self-Hosted on their own hardware, and their data location is identical whether users open a browser, the installable PWA, or the desktop app. The client line therefore stays web-first — one TypeScript codebase serving browser, PWA, Tauri desktop, and mobile shells — with the desktop app adding what desktop users actually miss: tray presence, native notifications, and the offline-first local cache. *"Your data on your servers, your people on any screen."*

## 6. Migration: one move, complete

The half-migrated customer is the failure mode — still paying Microsoft, now paying twice. alo's launch bar is therefore: **a 30-person company can cancel its entire M365 subscription the day after migrating.**

alo Migrate covers:

- **Pre-migration audit** — scans the M365 tenant via Graph API, reports what is actually used, flags blockers (macro workbooks, Power Automate flows), outputs a readiness score and a cost-savings figure. Doubles as the sales tool.
- **Identity** — imports users and groups from Entra ID; alo becomes the OIDC/SAML identity provider so every other SaaS login keeps working and the hidden Microsoft lock-in dies.
- **Mail furniture** — mailboxes, folders, signatures, inbox rules, out-of-office states, aliases, distribution lists, shared mailboxes with delegation permissions, PST archive import.
- **Calendar continuity** — recurring events with exceptions, room/resource mailboxes, and rewriting of Teams meeting links in future events to alo Meet links.
- **Files with permissions** — OneDrive/SharePoint content plus its sharing structure, with a report of anything that could not be mapped.
- **Zero-config clients** — autodiscover/autoconfig so Outlook, Apple Mail, and phones configure themselves from an email address.
- **Cutover safety** — dual delivery during DNS propagation, read-only archive of the old tenant for a grace period, per-user rollback.
- **Day one** — the AI assistant answers "where did X go?" questions in-product; change management shipped as a feature.
- **Subscription retirement** — the wizard ends with a dependency check confirming nothing still requires M365, the monthly savings figure, and a generated cancellation checklist for the Microsoft admin portal.

For the rare immovable dependency (the accountant's macro workbook), the playbook offers desktop LibreOffice/OnlyOffice connected to Drive, or a one-time perpetual Office license — the files live in alo, and the recurring contract still dies.

## 7. Licensing and business model

**Third-party license audit (July 2026):** every integrated component is free for commercial SaaS and self-hosted use with nothing to purchase. Synapse, OnlyOffice CE, and Garage are AGPL-3.0 (obligation: offer their source to users and publish any patches — trivial when running unmodified pinned containers); Collabora Online is MPL-2.0 (publish modified files only, rebrand for trademark); LiveKit, Rspamd, Qdrant are Apache 2.0; PostgreSQL and pgvector carry the permissive PostgreSQL license; Tauri is MIT/Apache. Two standing compliance rules: AGPL components run as separate processes behind network APIs, never linked into alo binaries; and no upstream trademarks ship in the product. **Editors (ADR 0033):** Collabora is removed; the editor engines are now embedded TypeScript **libraries** — **Univer** (Apache-2.0) for Sheets, **BlockNote** (MPL-2.0) for Docs, with KaTeX/Prism (MIT) for math/code — carrying no AGPL obligation and no container. As libraries under our own UI they are a build-on-open-framework choice, not a whole-app fork.

- **Open source core** under AGPL-3.0, with a commercial license available (dual licensing) — the model proven by comparable European companies. Openness is the sovereignty pitch made verifiable; AGPL ensures competitors who host the code must publish their changes.
- The multi-tenant control plane and billing may remain proprietary (open-core boundary to be fixed before launch).
- **Revenue:** alo Cloud subscriptions (priced visibly below the customer's current M365 tier, AI included rather than a €30/user add-on), self-hosted licenses and support, and MSP/reseller margin — the channel that made Odoo win, applied to communication software.

## 8. Roadmap

| Phase | Months | Delivers | Milestone |
|---|---|---|---|
| 0 — Foundations | 0–1 | Legal/IP, licensing decision, trademark, repo, CI, EU hosting | First SMTP message accepted on a test domain |
| 1 — Mail core | 1–7 | SMTP, auth stack, store, JMAP, IMAP shim, Sieve, identity | Founder lives on alo mail daily |
| 2 — Product layer | 6–12 | Webmail, admin + deliverability autopilot, Agenda, Chat, Meet, Drive + Docs, control plane | First Axon company fully cut over |
| 3 — AI layer | 9–14 | Event bus, semantic index, EU LLM serving, triage/summarize/draft, attachment understanding, MCP | "What did I miss this week?" answered across mail, chat, meetings |
| 4 — Migration suite | 12–16 | Full alo Migrate as described above | A non-Axon pilot migrated in one weekend by their own IT |
| 5 — Launch | 16–18 | Remaining Axon companies as case studies, security audit, pricing, 2–3 MSP partners, public launch | Public availability; EAS/MAPI adapter work begins |

Exchange-compatibility adapters (ActiveSync, then MAPI) are the year-two battle: launch sells alo's own apps plus open protocols; native Outlook support lands after.

## 9. Market and competition

**Target customer:** European SMEs and mid-market organizations (roughly 10–250 seats) that want off Microsoft for cost, sovereignty, or trust reasons but cannot run infrastructure themselves — reached primarily through MSPs and IT resellers, who earn recurring margin. Axon Group's twelve companies are the founding reference deployment.

**Landscape:** the sovereign-mail field is crowded but split — old technology with Exchange compatibility (Grommunio, IceWarp, Zextras Carbonio), or modern technology without it (Stalwart); infrastructure vendors (Open-Xchange/Dovecot) serve telcos, not SMEs; hosted providers (Proton, Infomaniak, mailbox.org) offer no self-hosting or suite depth; Nextcloud has the sovereignty brand but weak mail. Nobody is AI-native: AI is at best a webmail bolt-on, never a tenant-local layer across mail, chat, meetings, and files. alo's square — modern core + suite completeness + AI-native, on EU terms — is unoccupied because it is the hardest to reach; the corresponding competitive risk is Stalwart adding Exchange adapters or an incumbent bolting on credible AI before launch.

## 10. Security and compliance

- **Encryption:** TLS everywhere in transit; encryption at rest for blobs and databases; per-tenant keys so one tenant's breach is not a platform breach. Matrix E2E encryption available for chat.
- **Isolation:** container-per-tenant chat instances, per-tenant buckets and database schemas, tenant enforcement at the gateway.
- **GDPR as product:** data residency guaranteed (EU regions, or the customer's own hardware), subject-access exports and retention policies in the admin console, processing records template shipped with the product.
- **NIS2 readiness:** audit logging, incident-response documentation, and patching SLAs designed for customers who are themselves in scope.
- **Verification:** external security audit and penetration test before public launch; source code publicly auditable — trust as a checkable fact.

## 11. Risks and mitigations

| Risk | Mitigation |
|---|---|
| Solo-founder bus factor on a from-scratch mail core | Second developer from Phase 2; ruthless documentation; boring, well-specified protocols |
| Exchange-compat adapters (EAS/MAPI) slip beyond year two | Launch does not depend on them: own apps + open protocols first; adapters are the year-two battle, priced into the roadmap |
| Deliverability reputation of SaaS IP ranges | IP warming starts in Phase 1, not at launch; deliverability autopilot in-product; clean-tenant policies |
| Scope creep across five products | Phase gates: nothing enters launch scope that is not in this document |
| Upstream engine failure or relicensing (the MinIO pattern) | Integration doctrine: unmodified engines behind our own APIs, swappable by design |
| A competitor reaches the AI-native square first | Speed on the differentiators (mail core + AI); the migration suite and MSP channel as second moat |

## 12. Engineering practices

**Testing and interoperability.** A mail server is judged by clients we don't control, so interop is a first-class discipline: RFC compliance suites in CI from week one (SMTP, JMAP, IMAP, DAV), plus a standing real-client matrix — current and n-1 Outlook, Apple Mail (macOS/iOS), Thunderbird, Gmail app via IMAP, mainstream Android mail — run against every release. A permanent staging tenant mirrors production, and every Axon company migration doubles as a structured test pass. Deliverability is tested outward too: seed-list checks against major providers before and after each MTA change.

**Backups and disaster recovery.** Targets, not vibes: RPO ≤ 24h for all tenant data (Postgres WAL archiving + Garage replication across two EU locations), RTO ≤ 4h for a full tenant restore. Restores are rehearsed monthly by script — an untested backup is a hope, not a backup. GDPR export and tenant deletion reuse the same tooling, so compliance paths stay exercised.

**Observability and reliability.** Public status page, per-tenant metrics (queue depth, delivery latency, bounce rate, storage), alerting on deliverability signals (blacklist appearance, DMARC failure spikes), 99.9% availability target for alo Cloud at launch — honest for a small team, raised as the team grows. Every incident gets a public post-mortem; for a trust product, transparency in failure is marketing.

**Supply chain:** secrets in a vault (never in config), signed releases, dependency audit and SBOM generation in CI.

## 13. AI model strategy

The AI layer is model-agnostic by design — an internal inference API (`alo-ai`) routing to interchangeable backends, because the model landscape shifts faster than any other dependency:

- **alo Cloud default:** EU-hosted open-weight models (e.g. Mistral family) served from our own GPUs or an EU inference provider under DPA — customer data never reaches US endpoints.
- **Self-Hosted:** the same layer pointing at the customer's own GPU (vLLM/Ollama-compatible), or gracefully degraded AI features without one.
- **Embeddings** run locally in-process where possible (small models, cheap) so semantic search works on every tier.
- **Contractual guarantees:** customer data is never used to train models, never leaves the tenant boundary in inference logs, and every AI feature is per-tenant switchable.
- **Cost discipline:** summarization and triage batched and cached; the expensive path (drafting) is user-invoked; AI gross margin tracked per tenant from the first pilot.

## 14. Non-goals

Written down because scope creep killed most of our predecessors. alo will **not** build: office editors, a video/WebRTC stack, a Matrix homeserver, an object store, device management (MDM), a CRM/ERP (that is Odoo's territory — integrate, don't compete), or a public federated social layer. Entries here are revisited only with revenue on the table and a written case — the default answer stays no.

**Revised (ADR 0018):** *consumer/personal email* is no longer a Non-goal. alo now also offers self-service **personal** addresses (e.g. `johnsmith@alomails.com`) on platform-operated domains, as a distinct product line alongside the B2B offering. It is built on the same sovereign stack and the same per-tenant isolation (one tenant per person), verification-gated, with consumer sending reputation isolated from B2B customers. This is a deliberate scope expansion, not a free-tier growth-hacking play: the sovereignty promise and the "no tracking, no ads, no dark patterns" rule apply to personal users too.

**Revised (ADR 0035):** *CRM/ERP* is no longer "Odoo's territory — integrate, don't compete." The goal has widened: **alo is the one place a business does its work**, and the operational backbone (billing, CRM, projects, accounting, inventory, HR) is now built **from scratch, in waves, AI-native** — see §3's Business modules and the ROADMAP's Business track. The revision is bounded by its own non-goals: **no payroll calculation, no tax filing, no from-scratch bank connections** (export or integrate instead), and no attempt to clone all of Odoo at once — one deep module at a time, each behind the same quality gates as mail.

**Revised (ADR 0033):** *office editors* is narrowed, not abandoned. alo still does **not** build a from-scratch office *engine* — formula evaluation, layout, rendering — which is where the scope-creep danger lives. It **integrates open-source editor frameworks** (Univer, Apache-2.0, for Sheets; BlockNote, MPL-2.0, for Docs) as embedded TypeScript libraries and builds only its **own UI, chrome, and import/export** on top: the integrate-don't-build rule of §4, now applied to editors in place of the heavier Collabora-via-WOPI shell (ADR 0010, superseded). The one genuine new build is a native **Slides** canvas, because no open engine covers it — a deliberate, separately-justified expansion, not a blanket licence to rebuild Office. Real `.docx`/`.xlsx`/`.pptx` files are imported best-effort into the native types; pixel-faithful round-trip to desktop Office is explicitly no longer promised.

## 15. Open decisions

Tracked here until closed: final name (alo vs Atelier — EUIPO search pending); exact open-core boundary (which control-plane components stay proprietary); CLA tooling; hosting partner (Hetzner vs OVH vs Scaleway); pricing tiers; second developer hire (target: Phase 2 start); Peppol access point — integrate a certified AP vs. become one (ADR 0035, decide during Wave B1); EU PSP for invoice payment links. *(Closed — Collabora vs OnlyOffice as docs engine: resolved by **ADR 0033**, which removes Collabora in favour of native editors on embedded open engines.)*

## 16. Positioning in one line

Everyone else built a cheaper Exchange. **alo is the first workspace where the mail thinks — open, European, and complete enough to cancel Microsoft.**
