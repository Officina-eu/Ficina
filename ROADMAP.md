# Ficina — ROADMAP.md

Progress is marked, never dated: a phase advances when its exit gate is fully
checked, however long that takes. Mark items [x] only when they meet the
definition of done in `.claude/skills/implement/` — full depth, gated,
documented. No item may be checked with a stub behind it.

Rules of this file:

- Work top to bottom inside a phase; phases may overlap only where marked ⇄.
- The exit gate is the phase. Unchecked gate = phase not done, regardless of
  how many items above it are checked.
- New items enter only through the scope gate (`features.md` tier + product
  doc Non-goals). Removing an item requires noting where it went (cut, moved,
  merged).

## Phase 0 — Foundations

### Legal & identity

- [ ] Company/IP structure decided and set up
- [ ] License model fixed: AGPL-3.0 core + commercial (per ADR 0002); open-core boundary written down
- [ ] CLA text chosen and signing tool wired (before the repo is public, not after)
- [ ] Name confirmed via EUIPO search (Ficina vs Atelier — Open Decisions closes)
- [ ] Trademark filed, classes 9 + 42
- [ ] Domains registered (.eu, .com, .io) + GitHub/Forgejo org + social handles

### Repo & infrastructure

- [x] Monorepo initialized: `core/` `web/` `control/` `migrate/` `deploy/` `docs/` + the governance layer (CLAUDE.md, skills, agents, ADRs)
- [x] CI: quality-gate commands run on every PR; releases build from tags only
- [ ] EU hosting partner selected (Open Decisions closes); first server live
- [x] `deploy/` composes the engine set (Synapse, LiveKit, Collabora, Garage, Postgres, Rspamd) at pinned versions
- [ ] Test domain configured: DNS, rDNS, first DKIM/SPF records (BLOCKED until constant-time credential compare lands — no public 587 with a timing oracle)
- [ ] IP warming begins now — sending reputation is grown for launch, not at launch

### Exit gate — Phase 0 done when:

- [ ] `ficina-smtp` (stub) accepts one message on the test domain, delivered through CI-built artifacts
- [ ] A stranger could clone the repo and understand the project from CLAUDE.md + docs alone

## Phase 1 — Mail core

### Receiving & sending

- [x] SMTP server: session state machine, EHLO negotiation, receive (25) and submission (587), per protocol skill non-negotiables
  - [x] Session state machine (RFC 5321 §4.1.4), EHLO/HELO negotiation, full receive path: MAIL/RCPT/DATA, §4.1.2 address parsing, dot-stuffing, Received: stamping, durable spool (M1; production port 25 binding lands with real deployment)
  - [x] Submission (587) with RFC 6409 rewrites (M3: Date/Message-ID fixups; From/Sender canonicalization deliberately out of scope)
- [x] Queueing: durable queue, retry schedule, 4xx/5xx semantics, bounce generation (DSN)
  - Deferred to M5: a mixed remote+local envelope holds its remote recipients' bounce until local delivery exists (held, not lost); DSN prose is not yet i18n'd
- [x] Outbound delivery: MX resolution + delivery, per-pass connection reuse
  - Deferred (not launch-blocking): cross-pass connection pooling and per-destination concurrency caps — delivery is currently sequential per pass; hardened when volume warrants
- [x] STARTTLS + AUTH; TLS enforced on submission (M3: STARTTLS/implicit-TLS via rustls, AUTH PLAIN/LOGIN over TLS on submission, truthful EHLO capabilities; credentials are a config-file dev bootstrap until ficina-identity in M9)
- [x] Size limits enforced during read; timeouts per RFC 5321

### Trust stack

Built as the `ficina-auth-mail` crate, wired into `ficina-smtp` at DATA
(inbound verdicts → `Received-SPF` + `Authentication-Results`, the
RFC 8601 contract) and at submission (DKIM signing). RSA crypto uses
`ring` (constant-time), not the `rsa` crate (RUSTSEC-2023-0071).

- [x] SPF verification (M4: RFC 7208 full `check_host` — all mechanisms, macro expansion, 10-DNS-lookup + 2-void-lookup hard limits)
- [x] DKIM: verification and signing, key management with rotation support (M4: verify multi-sig + relaxed/simple canon + `l=`/`x=`; sign RSA-2048 + Ed25519; `KeyStore` trait addressed by (domain, selector), file impl with perm checks + zeroize; verified by an independent tool (dkimpy))
- [x] DMARC evaluation + report generation (M4: PSL org-domain, relaxed/strict alignment, disposition→550 on `p=reject`, aggregate-report XML per Appendix C)
  - Deferred: DMARC report *delivery* (gzip + email via the M2 queue) — the XML is generated; sending is a follow-up job
- [ ] ARC sealing (needed for lists later)
  - Deferred from M4 (sanctioned cut seam): chain validation + AAR/AMS/AS sealing; needed for mailing-list forwarding, not first-hop receive/submit
- [ ] MTA-STS + TLS-RPT published for our domains
  - Deferred from M4: MTA-STS policy serving + TLS-RPT report JSON (TLS-RPT reporting is a sanctioned cut seam)
- [ ] Rspamd integrated at SMTP time; verdict wired to reply codes and headers
  - Deferred from M4: HTTP consult at DATA, verdict → 550/451/X-Spam headers merged into Authentication-Results, fail-closed default. The `x-spam` method is already reserved in the Authentication-Results builder

### Store & APIs

- [ ] `ficina-store`: mailboxes, messages, flags, threads, blobs on Postgres + Garage; full-text index
- [ ] Every store operation tenant-scoped; wrong-tenant test suite in place and required by CI
- [ ] JMAP: session, Mailbox/*, Email/get|query|set|changes, push (RFC 8620/8621)
- [ ] IMAP shim: LOGIN/SELECT/FETCH/STORE/SEARCH/IDLE against the store (9051/3501 compat)
- [ ] POP3 shim
- [ ] Sieve engine: base + vacation + subaddress; rules stored per user
- [ ] `ficina-identity` v1: users, groups, aliases, argon2 credentials, OIDC provider, 2FA (TOTP)

### Exit gate — Phase 1 done when:

- [ ] The founder's real daily mail runs on Ficina via Thunderbird + a raw JMAP client, for two continuous weeks, zero lost messages
- [ ] Interop pass recorded: Thunderbird, Apple Mail, Gmail-app-via-IMAP send/receive/flag/search correctly (transcripts in `docs/interop.md` where quirks surfaced)
- [ ] Deliverability: our mail reaches Gmail/Outlook.com/Proton inboxes (not spam) from the warmed IP

## Phase 2 — Product layer ⇄ (may overlap Phase 1 tail)

### Webmail & mail UX

- [ ] Web app shell: design system, auth flow, navigation — the one-product frame
- [ ] Mail: read/compose/reply, conversation view + flat toggle, folders/subfolders, drag-drop
- [ ] Organization primitives: flags with due dates, categories/colors, archive keystroke, unread counts
- [ ] Undo send, send later, snooze
- [ ] Visual Sieve rule builder
- [ ] Signatures (per identity + org footer), out-of-office with scheduling
- [ ] Search UI over the store index — fast enough to feel local
- [ ] PWA installable: offline shell, push notifications

### Agenda

- [ ] `ficina-dav`: CalDAV/CardDAV over the store
- [ ] Events, invitations (iTIP/iMIP), free/busy, recurring events with exceptions (the interop minefield — its own test corpus)
- [ ] Shared calendars, room/resource booking
- [ ] Agenda UI integrated with Mail (invite cards) and later Meet

### Chat & Meet

- [ ] Synapse per tenant provisioned by control plane; OIDC delegated to `ficina-identity`
- [ ] Ficina Chat UI: channels, DMs, threads, reactions, mentions, guest access — Matrix invisible
- [ ] Application service streaming events to the (future) AI bus
- [ ] LiveKit deployed; token minting from Ficina identities; Meet UI on the components SDK
- [ ] Meeting links native in Agenda; recording to Drive with consent indicators

### Drive & Docs

- [ ] Drive: spaces, permissions, share links (password/expiry), trash/restore, version history
- [ ] WOPI endpoints (CheckFileInfo/GetFile/PutFile) served by Drive
- [ ] Collabora embedded, Ficina-themed; Docs/Sheets/Slides live per `features.md`
- [ ] Fidelity CI: real-document corpus round-trips to desktop Office without mangling
- [ ] Desktop sync client v1

### Platform

- [ ] Admin console: tenants, users, domains, quotas
- [ ] Deliverability autopilot v1: DNS wizard, DKIM rotation, DMARC monitoring, blacklist alerts
- [ ] Multi-tenant control plane: provisioning APIs for every engine, billing hooks
- [ ] Native distribution lists + shared mailboxes with delegation
- [ ] Backups per DR targets; restore rehearsal scripted and passing
- [ ] Audit log; GDPR export; tenant export (exit as a feature)

### Exit gate — Phase 2 done when:

- [ ] Axon company #1 fully cut over: every employee's mail, calendar, chat, meetings, files on Ficina
- [ ] One non-technical Axon user, asked how many products they're using, answers "one"
- [ ] A restore rehearsal recovered a full tenant within the RTO target

## Phase 3 — AI layer ⇄ (may overlap Phase 2)

- [ ] Event bus: store/chat/calendar/file events flowing to indexers
- [ ] Per-tenant semantic index (embeddings local; pgvector first per ADR)
- [ ] Model-agnostic inference API; EU-hosted open-weight default; Self-Hosted GPU path; per-tenant AI off-switch
- [ ] Semantic search across mail/chat/files in one query bar
- [ ] Thread summarization; drafted replies in user tone (user-invoked)
- [ ] Attachment understanding: incoming .docx/.xlsx readable and summarizable
- [ ] Daily digest: "what did I miss" across all modules
- [ ] Inbox triage v1, per-user trainable
- [ ] Meeting minutes: transcript → summary/decisions/actions posted to the meeting's chat thread
- [ ] MCP server with per-agent permissions
- [ ] "Where did X go?" onboarding assistant
- [ ] Contractual guarantees implemented and verifiable: no training on customer data, no inference logs crossing tenant boundary

### Exit gate — Phase 3 done when:

- [ ] The demo runs live on real Axon data: "what did I miss this week?" answered correctly across mail, chat, and meetings
- [ ] AI cost per tenant measured and within the pricing model's margin

## Phase 4 — Migration suite

- [ ] Tenant audit tool: Graph API scan → usage report, blocker flags (macros, Power Automate), readiness score, savings figure
- [ ] Identity import from Entra ID; Ficina as IdP for the customer's other SaaS
- [ ] Mailbox import: mail, folders, rules, signatures, OOF, aliases; PST import
- [ ] Shared mailboxes + delegation permissions mapped
- [ ] Calendar import: recurrences with exceptions, rooms/resources; Teams links in future events rewritten to Ficina Meet
- [ ] Contacts import
- [ ] OneDrive/SharePoint import with permission mapping + unmappable-items report
- [ ] Autodiscover/autoconfig endpoints: clients self-configure from an email address
- [ ] Cutover safety: dual delivery during DNS propagation, read-only archive of old tenant, per-user rollback
- [ ] Subscription retirement screen: dependency check, savings figure, cancellation checklist

### Exit gate — Phase 4 done when:

- [ ] A non-Axon pilot company is migrated in one weekend by their own IT person — Ficina staff observing, not driving
- [ ] The pilot cancels (or formally schedules cancellation of) their M365 subscription

## Phase 5 — Launch

- [ ] Remaining Axon companies migrated; two written as public case studies
- [ ] External security audit + penetration test; findings fixed; report summarized publicly
- [ ] Pricing published (Cloud below the M365 tier it replaces, AI included; Self-Hosted license)
- [ ] Public source page live: our repos + upstream engines + versions (AGPL compliance + the trust story)
- [ ] Status page, public post-mortem policy, security.txt, disclosure policy
- [ ] Docs site: admin guide, migration playbook (incl. the VBA/desktop-Office answer), API/MCP reference
- [ ] 2–3 Belgian/EU MSP partners signed with reseller margin; white-label mode v1
- [ ] Support channel + SLA definitions per tier

### Exit gate — Phase 5 done when:

- [ ] A customer with no prior relationship to the founder signs, migrates, and pays
- [ ] The company survives the founder taking one full week off (runbooks + monitoring prove it)

## Phase 6 — Year-two battles (post-launch, ordered)

- [ ] EAS adapter: phones sync natively (mail/calendar/contacts) against the JMAP core
- [ ] Fast-follow features from `features.md` tier [2]: booking pages, meeting polls, shared-inbox collaboration, auto-translation, follow-up nudges, smart folders, internal recall, huddles, AI digest hardening
- [ ] Tauri desktop shell: tray, notifications, autostart
- [ ] Offline-first local cache (design review first, per ADR 0005)
- [ ] Mobile apps
- [ ] MAPI-over-HTTP adapter: native Outlook — the last wall
- [ ] Second developer hired and shipping independently (bus factor > 1 proven by a release the founder didn't cut)

### Exit gate — Phase 6 done when:

- [ ] An Outlook desktop user works a full week against Ficina without knowing Exchange is gone

---

When every box in a phase is checked, mark the phase header — DONE and record
the date of the gate in git history, not in this file. The file stays about
what; git stays about when.
