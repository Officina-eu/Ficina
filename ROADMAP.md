# alo — ROADMAP.md

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
- [ ] Name confirmed via EUIPO search (alo vs Atelier — Open Decisions closes)
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

- [ ] `alo-smtp` (stub) accepts one message on the test domain, delivered through CI-built artifacts
- [ ] A stranger could clone the repo and understand the project from CLAUDE.md + docs alone

## Phase 1 — Mail core

### Receiving & sending

- [x] SMTP server: session state machine, EHLO negotiation, receive (25) and submission (587), per protocol skill non-negotiables
  - [x] Session state machine (RFC 5321 §4.1.4), EHLO/HELO negotiation, full receive path: MAIL/RCPT/DATA, §4.1.2 address parsing, dot-stuffing, Received: stamping, durable spool (M1; production port 25 binding lands with real deployment)
  - [x] Submission (587) with RFC 6409 rewrites (M3: Date/Message-ID fixups; From/Sender canonicalization deliberately out of scope)
- [x] Queueing: durable queue, retry schedule, 4xx/5xx semantics, bounce generation (DSN)
  - Local delivery now exists (see "Local delivery" below), so a mixed remote+local envelope's local recipients are filed directly; the remaining deferral is holding the *remote* recipients' bounce for a mixed envelope and i18n'ing the DSN prose
- [x] Outbound delivery: MX resolution + delivery, per-pass connection reuse
  - Deferred (not launch-blocking): cross-pass connection pooling and per-destination concurrency caps — delivery is currently sequential per pass; hardened when volume warrants
- [x] STARTTLS + AUTH; TLS enforced on submission (M3: STARTTLS/implicit-TLS via rustls, AUTH PLAIN/LOGIN over TLS on submission, truthful EHLO capabilities; credentials are a config-file dev bootstrap until alo-identity in M9)
- [x] Size limits enforced during read; timeouts per RFC 5321
- [x] Local delivery: inbound SMTP files into the account store with Sieve at the boundary — recipients resolved via `Store::account_by_email` at RCPT (unknown local user → `550 5.1.1` at RCPT, not after DATA), each resolved recipient delivered through `AccountStore::deliver_sieve` (parse → spam score → Sieve → file), Sieve `redirect`/`vacation` enqueued through the M2 outbound queue with CR/LF-stripped headers, per-recipient try-then-commit with a conservative whole-message `4xx` on transient store faults (no mail loss), durable on-disk blobs (`BlobStore::local`, `ALO_SMTP_BLOB_DIR`); the inbound spool is retired as the local sink and its all-local backlog migrated into the store once at startup. Design note `docs/design/local-delivery.md`; reviewed + security-audited

### Trust stack

Built as the `alo-auth-mail` crate, wired into `alo-smtp` at DATA
(inbound verdicts → `Received-SPF` + `Authentication-Results`, the
RFC 8601 contract) and at submission (DKIM signing). RSA crypto uses
`ring` (constant-time), not the `rsa` crate (RUSTSEC-2023-0071).

- [x] SPF verification (M4: RFC 7208 full `check_host` — all mechanisms, macro expansion, 10-DNS-lookup + 2-void-lookup hard limits)
- [x] DKIM: verification and signing, key management with rotation support (M4: verify multi-sig + relaxed/simple canon + `l=`/`x=`; sign RSA-2048 + Ed25519; `KeyStore` trait addressed by (domain, selector), file impl with perm checks + zeroize; verified by an independent tool (dkimpy))
- [x] DMARC evaluation + report generation (M4: PSL org-domain, relaxed/strict alignment, disposition→550 on `p=reject`, aggregate-report XML per Appendix C)
  - [x] DMARC report *delivery* (per-domain daily windows from recorded MX evaluations → gzip → §7.2.1.1 report mail via the outbound queue, §7.1 external-destination verification, DKIM-signed; `ALO_SMTP_DMARC_REPORTS=off` kill switch)
  - Deferred: TLS-RPT (`_smtp._tls` JSON) report delivery — needs per-policy TLS session outcome tracking in the outbound client first
- [x] ARC sealing (RFC 8617 first hop `i=1; cv=none` on Sieve-redirect forwards, sealed with the forwarding tenant's DKIM key; chain validator cross-checked against dkimpy both directions; `ALO_SMTP_ARC_SEALING=off` kill switch)
  - Deferred: inbound `arc=` stamping in Authentication-Results + sealing onto an existing chain (`i>1`) — the validator exists; wiring it at ingress is a follow-up
- [x] DANE for outbound (RFC 7672: per-MX `_25._tcp` TLSA over a DNSSEC-validating resolver — hickory validates the chain itself; secure usable set → mandatory DANE-EE-verified TLS, secure unusable set → mandatory unauthenticated TLS, TLSA lookup failure → host skipped, never downgraded; `ALO_SMTP_DANE=off` kill switch)
  - Deferred: DANE-TA(2) chain building (such records count as unusable → TLS still mandatory); secure-MX gating per §2.2.1 (we enforce on any secure TLSA — strictly stronger, recorded in `resolver.rs`)
  - Deferred from M4 (sanctioned cut seam): chain validation + AAR/AMS/AS sealing; needed for mailing-list forwarding, not first-hop receive/submit
- [x] MTA-STS published for our domains (M4b: RFC 8461 policy rendered from config — mode/mx/max_age, content-derived `id` — and served at `GET /.well-known/mta-sts.txt` behind the deploy TLS proxy; DNS records documented)
  - Deferred (sanctioned cut seam): TLS-RPT report JSON (`_smtp._tls` reporting)
- [x] Rspamd integrated at SMTP time; verdict wired to reply codes and headers (M4b: HTTP `/checkv2` consult at DATA → reject=550, soft-reject/greylist=451, else accept with `x-spam` merged into Authentication-Results; **fail-closed 451** when the scanner is unreachable; verified end-to-end with real Rspamd 4.1.2 — GTUBE → 550)
  - [x] Junk training: `Email/set` moves into/out of the Junk role call the Rspamd controller's learnspam/learnham (best-effort spawned, never gates the move; `ALO_JMAP_RSPAMD_URL` env, off when unset). Deploy gains a pinned redis (Bayes token store — previously Bayes was silently dead: no backend) + `secure_ip` controller access on the private network
  - [x] ClamAV malware scanning at DATA (clamd INSTREAM over the private network, pinned `clamav/clamav:1.4.5` with a persistent signature volume; signature match → 550 5.7.1 with the sanitized signature name, scanner outage → **fail-closed 451** exactly like Rspamd; >20 MiB messages pass unscanned, loudly logged; `ALO_SMTP_CLAMAV_ADDR` env, off when unset)
  - [x] Abuse controls: native per-source-IP concurrent-connection cap (accept-loop, IPv6 bucketed by /64, over cap → 421; `ALO_SMTP_MAX_CONNECTIONS_PER_IP`, default 20) + greylisting (Rspamd greylist module, now redis-backed → `soft reject`/451 for unknown triplets) + outbound per-destination-domain send-rate limiting (token bucket in the queue, over-rate → defer-not-bounce, protects sending-IP reputation; `ALO_SMTP_OUTBOUND_RATE_PER_MIN`/`_BURST`, off by default)
  - Deferred: FBL/ARF complaint handling (needs an inbound `abuse@`/complaint mailbox + RFC 5965 ARF parsing → auto-suppress; its own item)

### Store & APIs

- [x] `alo-store`: mailboxes, messages, flags, threads, blobs on Postgres + Garage; full-text index (opaque JMAP ids, hierarchical mailboxes with transactional counters, References threading, content-addressed blobs per-tenant, parsed Authentication-Results stored queryable, Postgres `tsvector` FTS; sqlx compile-checked with an offline cache; ADR 0006)
  - Deferred: the Garage S3 *live-integration* test (backend is behind the `garage` feature and compiles; in-memory backend is tested). The one-way spool-migration seam is now built as the local-delivery startup pass (all-local spool entries migrate into the store before the queue runner starts); a general mixed inbound/outbound backlog migration remains out of scope
  - A durable on-disk blob backend (`BlobStore::local`, object-store's filesystem store) now backs single-node local delivery, so a delivered body survives a restart without requiring Garage/S3
- [x] Every store operation tenant-scoped; wrong-tenant test suite in place and required by CI (tenancy by construction — a `TenantStore` is the only door and bakes the tenant predicate into every query; the isolation suite covers every public read/write path and CI runs it against real Postgres)
- [x] JMAP: session, Mailbox/*, Email/get|query|set|changes, push (RFC 8620/8621) (`alo-jmap`: Session with honest enforced limits, request batching + result references, interim bearer auth (argon2, trait-swappable for OIDC), Mailbox/Email/Thread get/set/query/changes with per-tenant modseq state tokens, blob upload/download, EventSource push; ADR-style design note; wrong-tenant suite extended to every method + blob + push, CI-gated)
  - Deferred (sanctioned cut seam): `EmailSubmission/set` (h) — sending a draft through the M2/M3 queue; recorded for the next pass. Also additive-later: full MIME `bodyStructure`/attachments in `Email/get`, JMAP-over-WebSocket (RFC 8887)
- [x] IMAP shim: LOGIN/SELECT/FETCH/STORE/SEARCH/IDLE against the store (9051/3501 compat) (`alo-imap`: implicit-TLS 993 + STARTTLS 143, LOGIN/AUTHENTICATE over TLS only with a failed-auth cap; full mailbox + message command set incl. UID variants, COPY/MOVE (6851), APPEND through the one ingestion path, special-use LIST (6154), and IDLE (2177) as account-scoped push; stable per-mailbox UIDs + UIDVALIDITY (migration 0006); byte-exact FETCH body sections + bounded-honest BODYSTRUCTURE; cross-tenant AND cross-account isolation suite over real TLS; reviewed + security-audited)
  - Deferred (additive later, named in `docs/design/imap-pop3-shims.md`): CONDSTORE/QRESYNC, SORT/THREAD, ACL/QUOTA/METADATA/COMPRESS/BINARY, sub-second IDLE via LISTEN/NOTIFY; a Thunderbird desktop-GUI interop pass (imaplib + openssl transcript stand in for this milestone)
- [x] POP3 shim (`alo-imap::pop3`: implicit-TLS 995, USER/PASS via the same credential seam, STAT/LIST/RETR/DELE/RSET/NOOP/QUIT/UIDL/TOP, inbox-only, UIDL reusing the stable IMAP UIDs, deletion committed on QUIT)
- [x] Sieve engine: base + vacation + subaddress; rules stored per user (`alo-sieve`: RFC 5228 parser/AST/evaluator with `require` enforcement and hard parse limits + an instruction budget — all security controls; actions keep/fileinto/discard/redirect/stop with implicit keep and mail-never-lost on any failure; vacation (5230) with RFC 3834 guards + per-correspondent `:days` suppression, subaddress (5233), imap4flags (5232) mapped to store keywords. Wired at the store delivery entry `AccountStore::deliver_sieve` (after spam scoring, before filing), with per-account script storage, vacation suppression, and a per-account redirect rate budget — all account-scoped, isolation inherited. Rule management is JMAP for Sieve (RFC 9661, ADR 0007): `SieveScript/{get,set,validate}` compile-checked on set. Reviewed + security-audited.)
  - The SMTP → mailbox **local-delivery bridge** that turns swaks into a filed message is now built (see "Local delivery" under "Receiving & sending"): the seam (`Store::account_by_email` + `deliver_sieve`, returning redirect/vacation `OutboundAction`s) is exercised on the real inbound path, with Sieve outbound actions enqueued through the M2 queue. Also additive-later, per `docs/design/sieve-filtering.md`: ManageSieve, `body`/`regex`/`variables`/`relational`/`date`/`notify`/`include` extensions, a per-account vacation-send budget, and blob-based (vs inline) SieveScript content.
- [x] `alo-identity` v1: users, groups, aliases, argon2 credentials, OIDC provider, 2FA (TOTP) (`alo-identity` is the credential authority behind SMTP AUTH, IMAP/POP3 `LOGIN`, and the JMAP bearer — the interim `StaticAuthenticator`, the store's `auth.rs`, and the SMTP credentials-file loader are **deleted**. **argon2id** password hashing with a documented parameter contract and a **constant-time** verify plus **dummy-hash** for unknown users, closing the pinned M3 timing oracle — proven by a timing test, `ratio≈1.0`. Every secret compared constant-time (`subtle`); tokens/recovery codes stored only as SHA-256 hashes. Identity model: tenants → users → aliases + groups; `account_by_email` is **alias-aware**; admin bootstrap is a CLI (`identityctl`), never a public endpoint. **OIDC/OAuth 2.0 provider**: discovery (RFC 8414), JWKS, `authorization_code` + **PKCE S256** (RFC 6749/7636), token + userinfo + revoke (RFC 7009); **opaque revocable access tokens**, rotated refresh tokens with replay-chain revocation, **EdDSA** ID tokens (ADR 0008). **TOTP 2FA** (RFC 6238) with drift window + single-use recovery codes; enforced everywhere — the OIDC flow prompts for the code, and the legacy protocols (which cannot) **fail closed** for a 2FA account, so a phished password cannot bypass 2FA over IMAP. Per-`(client,)username` backoff on the token endpoints **and** the legacy auth path. Reviewed + security-audited across **two independent passes**; cross-tenant AND cross-account isolation tested on every identity operation, plus OAuth negative-path coverage (wrong PKCE verifier, code/refresh replay → chain revoke, unregistered redirect). Design note `docs/design/identity.md`, ADR 0008)
  - Sanctioned cut seam (named in the design note): app-specific passwords + `XOAUTH2` on submission (how a 2FA user drives a legacy IMAP/SMTP client — the interim is account-password-on-legacy, TOTP enforced on the browser flow). Also deferred: binding submission `MAIL FROM` to the authenticated identity (send-as), which needs the group/alias permission model this milestone ships the data for; confidential clients, dynamic client registration, device/client-credentials grants, WebAuthn/passkeys, and an admin HTTP console

### Exit gate — Phase 1 done when:

- [ ] The founder's real daily mail runs on alo via Thunderbird + a raw JMAP client, for two continuous weeks, zero lost messages
- [~] Interop pass recorded: Thunderbird, Apple Mail, Gmail-app-via-IMAP send/receive/flag/search correctly (transcripts in `docs/interop.md` where quirks surfaced).
  **Protocol-level loop verified on prod** (2026-08-02, IMAPS+SMTPS): LOGIN/SELECT/
  SEARCH ALL/SEARCH SUBJECT (quoted)/STORE flag + AUTH/SEND; send→receive→search in
  ~6s; imaplib multi-word-quoting quirk recorded in `docs/interop.md`. Remaining: the
  GUI-client matrix (the actual Thunderbird/Apple Mail/Gmail apps)
- [~] Deliverability: our mail reaches Gmail/Outlook.com/Proton inboxes (not spam) from the warmed IP.
  **Trust stack verified on prod** (SPF/DKIM/DMARC/MX all pass, strict alignment).
  **Blocker: PTR (reverse DNS) is unset** — must be set at the IP/hosting provider;
  it dominates inbox placement. MTA-STS optional/unpublished. External-inbox receipt
  test pending the PTR fix (see `docs/interop.md`)

## Phase 2 — Product layer ⇄ (may overlap Phase 1 tail)

### Webmail & mail UX

- [x] Web app shell: design system, auth flow, navigation — the one-product frame (React/Vite/TS app: "warm workshop" design tokens + self-hosted Inter/EB Garamond + shared primitives; the left rail + layout frame + a module registry that makes Agenda/Chat/Drive/Docs one entry each; first-party OIDC authorization-code + PKCE login against `alo-identity` with 2FA-on-demand; a typed JMAP client with bearer + transparent refresh. Served at the same origin as the API behind Caddy (SPA + `/oauth`+`/jmap`+`/.well-known` proxied), strict CSP; login verified end-to-end on the live deployment. Design note `docs/design/web-shell.md`)
  - This item is the *frame*; the mail body below it is read-only so far (folders → message list → reading pane), with compose/reply and the rest as their own items
- [ ] Mail: read/compose/reply, conversation view + flat toggle, folders/subfolders, drag-drop
- [ ] Organization primitives: flags with due dates, categories/colors, archive keystroke, unread counts
- [ ] Undo send, send later, snooze
- [ ] Visual Sieve rule builder
- [ ] Signatures (per identity + org footer), out-of-office with scheduling
- [ ] Search UI over the store index — fast enough to feel local
- [x] Responsive / phone layout for Mail: below 768px the three-pane view becomes single-pane list↔detail (a `useIsMobile` matchMedia hook drives it) — folders slide in as an off-canvas drawer (toggled from the list header, closes on selection), the reading pane gets a back-to-list control, resize handles are hidden, and the reading toolbar wraps. Desktop unchanged
- [ ] PWA installable: offline shell, push notifications
- [~] Localization (i18n): runtime locale mechanism built — `strings` is a proxy over the active catalog so all ~50 call sites switch language with zero changes, English fallback per key (a partial catalog never blanks), browser detection + localStorage persistence + `<html lang>`, language switcher in the account menu, full-tree remount on switch. **French catalog complete** (the whole ~600-key surface, native quality). Follow-on: NL + DE catalogs (data only, mechanism ready); server-synced per-user preference (currently client-side)

### Agenda

- [~] Contacts (address book): backend built — a tenant/user-scoped `contacts` store (migration 0034, multi-valued emails/phones as JSONB), full vCard 4.0 round-trip (`alo-store::vcard`: FN/N/EMAIL/TEL/ORG/TITLE/NOTE/UID, folding + escaping, lenient parse), and a JMAP Contacts API (`Contact/get`/`Contact/set`, `urn:ietf:params:jmap:contacts`) with tenant isolation tested at both the store and API layers. Saved contacts now surface first in compose recipient autocomplete. **Web address-book UI built** — a two-pane modal (searchable list + editable detail) opened from the account menu, with create/edit/delete, dynamic multi-value emails/phones with type labels, server-derived display name, fully localized (EN + FR). vCard import/export is built (see "Contacts import/export"). **CardDAV device sync built** (see "alo-dav" below)
- [~] `alo-dav`: CalDAV/CardDAV over the store. **CardDAV built** (contact sync, RFC 6352): OPTIONS/PROPFIND/REPORT(multiget+sync-collection)/GET/PUT/DELETE at `/dav/…`, HTTP Basic via `authenticate_legacy`, RFC 6578 sync-token = account modseq, content-hash ETags, tenant-isolated (tested store + protocol). Currently a module in alo-jmap; the crate extraction + CalDAV (calendar) remain. Cut: `addressbook-query` filters (unfiltered fallback), PROPPATCH/MKCOL. Details in `docs/interop.md`
- [ ] Events, invitations (iTIP/iMIP), free/busy, recurring events with exceptions (the interop minefield — its own test corpus)
- [ ] Shared calendars, room/resource booking
- [ ] Agenda UI integrated with Mail (invite cards) and later Meet

### Chat & Meet

- [ ] Synapse per tenant provisioned by control plane; OIDC delegated to `alo-identity`
- [ ] alo Chat UI: channels, DMs, threads, reactions, mentions, guest access — Matrix invisible.
  Design bar = **Sila (silahq.com)**: sidebar (DMs/channels/agents/shared/search), polished
  message feed (avatars, bubbles, timestamps, media previews, presence). See `features.md` → Chat.
- [ ] ★ Agent-native chat: AI agents as first-class participants (own avatars, @mentionable,
  propose-then-approve replies/reactions) — the AI-native differentiator applied to chat.
- [ ] Application service streaming events to the (future) AI bus
- [ ] LiveKit deployed; token minting from alo identities; Meet UI on the components SDK
- [ ] Meeting links native in Agenda; recording to Drive with consent indicators

### Drive & Docs

- [ ] Drive: spaces, permissions, share links (password/expiry), trash/restore, version history
- [ ] Native editors on embedded open engines (ADR 0033, replaces Collabora/WOPI):
  - [x] alo Sheets — Univer engine + alo's own ribbon UI; real `.xlsx` import + `.xlsx` export
  - [ ] alo Docs — BlockNote; real `.docx` best-effort import
  - [ ] alo Slides — native in-house canvas; real `.pptx` best-effort import
  - [ ] Remove Collabora + WOPI once every format has a native home
- [ ] Technical authoring: browser-local math (KaTeX) + code (Prism) + alo auto-numbering/cross-references — standalone module first, docks into the Docs shell (ADR 0015)
- [ ] Fidelity CI: real-document corpus round-trips to desktop Office without mangling
- [ ] Desktop sync client v1

### Platform

- [x] Admin console: tenants, users, domains, quotas
- [ ] Deliverability autopilot v1: DNS wizard, DKIM rotation, DMARC monitoring, blacklist alerts
- [ ] Multi-tenant control plane: provisioning APIs for every engine, billing hooks
      — tenant lifecycle, domain ownership, quotas, and the operator surface
      (`alo-control`) landed (ADR 0012); per-engine provisioning + billing remain
- [ ] Native distribution lists + shared mailboxes with delegation
- [ ] Backups per DR targets; restore rehearsal scripted and passing
- [ ] Audit log; GDPR export; tenant export (exit as a feature)
      — tenant audit log landed (ADR 0012); GDPR + tenant export remain

### Exit gate — Phase 2 done when:

- [ ] Axon company #1 fully cut over: every employee's mail, calendar, chat, meetings, files on alo
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

### Docs & Sheets AI (editor-native)

Depends on the native editors (Phase 2, "Drive & Docs") **and** the AI
layer above — the editors are alo's own UI on embedded open engines
(Univer/BlockNote, ADR 0033, superseding the Collabora shell of ADR 0010),
so these ship after the core suite, never in Phase 1. UX source of truth:
Figma pages "10 · Docs" and "11 · Sheets". Trust model throughout: **the AI
proposes and diffs; the user accepts** — never a silent overwrite of a
document or a formula.

- [ ] Docs — clean paste: strip foreign formatting by default on external paste and match destination styles; dismissible "formatting cleaned — keep original" toast
- [ ] Docs — Ask-AI-from-your-docs: in-editor panel answering from the user's real files/workspace (not just the open doc), every answer **source-cited**; cross-suite (Mail/Drive/Calendar); suggested actions (insert / summarize)
- [ ] Docs — agentic AI: inline command bar (rewrite / shorten / fix grammar / custom on a selection); AI changes shown as an **accept/reject inline diff**; agent mode for multi-step tasks as a **visible plan** (per-step done/doing/pending, live progress, workspace grounding, Stop control)
- [ ] Docs — semantic-conflict flag: AI detects co-edits whose *meaning* no longer reconciles (e.g. a unit price vs a total) and surfaces an inline flag with keep-A / keep-B / let-me-fix
- [ ] Docs — draft-from-workspace-context: on an empty doc, list the sources it will use (email thread, meeting recording + AI notes, related sheets) and generate a first draft from them — the cross-suite killer move
- [ ] Sheets — explain-and-fix errors: plain-language card for #REF!/#VALUE!/#NAME? (why it broke + one-click fixes), AI proposes / user accepts
- [ ] Sheets — natural-language formulas: English → the **actual formula, shown and explained** (transparent and auditable — never a black box)
- [ ] Sheets — formula paste-guard: warn before a raw value overwrites a formula cell (paste-as-value vs keep-formula)
- [ ] Sheets — ask-your-data: NL question → answer with the **source cells cited + highlighted + a chart**; cross-suite; optional agent mode for multi-step data tasks with a visible plan + approval

### Exit gate — Phase 3 done when:

- [ ] The demo runs live on real Axon data: "what did I miss this week?" answered correctly across mail, chat, and meetings
- [ ] AI cost per tenant measured and within the pricing model's margin

## Phase 4 — Migration suite

- [ ] Tenant audit tool: Graph API scan → usage report, blocker flags (macros, Power Automate), readiness score, savings figure
- [ ] Identity import from Entra ID; alo as IdP for the customer's other SaaS
- [~] Mailbox import: mail, folders, rules, signatures, OOF, aliases; PST import.
  **IMAP mail import built, all folders + flags** — an account-menu wizard
  (Gmail/Outlook presets + any IMAP server) and `POST /import/imap`
  (`imap_import`): SSRF-guarded connect, verified-TLS (webpki-roots) implicit-TLS
  client, `LIST` + per-folder `SELECT`/`FETCH (FLAGS BODY.PEEK[])` of the newest
  ≤500 messages across all selectable folders. Folder structure preserved
  (special-use → role mailbox, others created by leaf name; Gmail virtual
  `\All`/`\Flagged`/`\Important` skipped to avoid double-import); flags carried
  over (`\Seen`/`\Flagged`/`\Answered`/`\Draft` → JMAP keywords); idempotent
  `Message-ID` dedup (per-run + against the store); honest imported/skipped/failed;
  auth refusal → 401 app-password hint. EN + FR. Wire-verified on prod.
  Remaining: modified-UTF-7 folder-name decoding, rules/signatures/OOF/aliases, PST
- [ ] Shared mailboxes + delegation permissions mapped
- [ ] Calendar import: recurrences with exceptions, rooms/resources; Teams links in future events rewritten to alo Meet
- [x] Contacts import/export: `.vcf` (vCard 4.0) bulk import (`POST /contacts/import`, multi-card split + per-card cap, honest imported/skipped count) and whole-address-book export (`GET /contacts/export`, a `text/vcard` attachment), wired into the address-book UI (Import/Export buttons, EN + FR). Migrates a Gmail/Outlook/Apple contacts export straight in
- [ ] OneDrive/SharePoint import with permission mapping + unmappable-items report
- [~] Autodiscover/autoconfig endpoints: clients self-configure from an email address.
  **Endpoints built** (`autoconfig`): Mozilla `clientConfig` (Thunderbird/Apple Mail)
  at `/.well-known/autoconfig/mail/config-v1.1.xml` + `/mail/config-v1.1.xml`, and
  Microsoft POX Autodiscover (Outlook) at `/autodiscover/autodiscover.xml` (both
  casings) — unauthenticated, advertising IMAPS 993 + SMTPS 465 on the server FQDN,
  caller input XML-escaped + charset-validated. Wire-verified on prod. Remaining
  (operator/deploy): per-email-domain `autoconfig`/`autodiscover` DNS + Caddy vhosts
  (documented in deploy README) so real clients resolve it from the email domain
- [ ] Cutover safety: dual delivery during DNS propagation, read-only archive of old tenant, per-user rollback
- [ ] Subscription retirement screen: dependency check, savings figure, cancellation checklist

### Exit gate — Phase 4 done when:

- [ ] A non-Axon pilot company is migrated in one weekend by their own IT person — alo staff observing, not driving
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
- [ ] Remote support / screen control (AnyDesk/TeamViewer-class — the EU IT-management play: one sovereign suite instead of a bolted-on remote tool). **Integrate** a self-hostable engine (RustDesk primary candidate); never build the capture/stream/input-injection engine ourselves — the highest-CVE-density surface in the product (ADR 0009). alo owns the UI/UX, session brokering, auth, consent, and audit logging. Launches from **Chat** (primary: the 1:1 DM header + person-profile quick-actions, beside Meet/Call/Email — where "help me" conversations live) and **Meet** (secondary: an in-call control-bar button for take-over-while-talking); a dedicated Remote/Support **rail tab is deferred** until the feature needs its own session management, history, and audit views. Requirements: native per-device agent (browsers cannot grant OS-level control — a hard boundary), E2E-encrypted session, **explicit per-session consent before any input**, an audit-log entry in the controlled user's security log, instant termination by either party, and a self-hosted relay (no third-party cloud). Screen *share* (read-only) is already in Meet; this is remote *control*, correctly sequenced post-launch. UX source of truth: the Figma design (request access / consent prompt / active-control banner with stop-sharing)
- [ ] Second developer hired and shipping independently (bus factor > 1 proven by a release the founder didn't cut)

### Exit gate — Phase 6 done when:

- [ ] An Outlook desktop user works a full week against alo without knowing Exchange is gone

---

## Business track — the Work OS (ADR 0035) ⇄ runs alongside Phases 3–6

alo widens from "replace M365" to **the one place a business does its work**:
the SAP/Odoo operational backbone, built from scratch on alo Base + the store +
the agent framework. Waves ship one at a time, full depth; **each wave's agent
is part of its definition of done** (ADR 0034 — propose-then-approve, EU
models). Feature detail lives in `features.md` → Business modules; this file
tracks the build order and gates.

### Wave B1 — Billing: Quotes & Invoices (the EU e-invoicing wedge)

Slices, in order — each shippable and wire-verified alone:

- [ ] B1.1 Foundations: customer billing records (VAT ID, terms) on Contacts; product/price list; money as integer cents end-to-end
- [ ] B1.2 Invoices: record + lines + server-computed VAT/totals; draft → issue with gapless per-tenant legal numbering; immutable once issued; credit notes
- [ ] B1.3 Quotes: same line model; draft → sent → accepted/declined/expired; accept → invoice
- [ ] B1.4 PDF: branded invoice/quote PDF, sent via alo Mail
- [ ] B1.5 ★ E-invoice out: Factur-X + XRechnung/UBL, schematron-validated (EN 16931)
- [ ] B1.6 E-invoice in: inbound Factur-X/XRechnung → parsed bill record
- [ ] B1.7 Payments: mark paid / partial, overdue view, manual reminders; VAT summary per period
- [ ] B1.8 ★ Billing agent: create/convert/chase by plain language, propose-then-approve, wired into Ask alo
- [ ] B1.9 Peppol via a certified access point (integrate first — open decision on own membership)

### Exit gate — B1 done when:

- [ ] A real business (Axon Group) runs a full month's invoicing on alo: quotes sent, invoices issued with legal numbering, one credit note, an XRechnung accepted by a government/large-buyer portal, and the VAT summary handed to the accountant without a spreadsheet

### Wave B2 — CRM & Sales — deals on real email *(gate: B1 live with ≥1 real tenant)*
### Wave B3 — Projects & Timesheets — billable hours feed B1
### Wave B4 — Expenses & Accounting core — receipts, ledger, reconciliation, VAT
### Wave B5 — Purchasing & Inventory — products, stock, PO/SO chains
### Wave B6 — HR — records, leave, recruitment-lite (payroll calc = permanent non-goal)

Later waves (post-traction, unordered): manufacturing-lite, POS, subscriptions,
e-signature (eIDAS), marketing sends, storefront, DATEV/PSD2 integrations.

---

When every box in a phase is checked, mark the phase header — DONE and record
the date of the gate in git history, not in this file. The file stays about
what; git stays about when.
