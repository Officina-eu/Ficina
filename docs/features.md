# Ficina — features.md

Feature inventory per module. Three tiers, mapped to the roadmap:
**[L]** = launch (must exist to cancel M365) · **[2]** = fast-follow, first year after launch · **[3]** = later, revenue-funded.
**★** marks differentiators — features Microsoft either lacks, charges extra for, or does badly.

Rule of the file: nothing gets built that isn't listed here, and nothing gets listed without a tier. Additions go through the scope gate (product doc, Non-goals).

---

## Mail

- [L] Mailboxes, folders/labels, aliases, plus-addressing (`user+tag@`)
- [L] Nested subfolder hierarchy with drag-and-drop, unread counts per folder (Outlook muscle memory — non-negotiable)
- [L] Flags / follow-up marking with optional due date, and a "flagged" smart view
- [L] Categories: color tags, multiple per message, filterable — shared org-wide category sets optional
- [L] Conversation view (threaded) with per-user toggle back to flat list — both camps exist and both must be happy
- [L] Archive as a first-class one-keystroke action
- [2] Smart folders — saved searches that behave as folders ("unread from customers", "flagged this week")
- [2] ★ Internal recall that actually works — within a tenant we control the store, so recalling an unread internal message genuinely deletes it, unlike Exchange's famously fake recall
- [2] Quick steps: one-click multi-action macros (mark read + move + forward), the power-user retention feature
- [L] Shared mailboxes with delegation (send-as, send-on-behalf)
- [L] Native distribution lists (the Mailman replacement — no bolt-on)
- [L] Server-side rules (Sieve) with a visual rule builder
- [L] Signatures (per-identity, org-enforced footer option)
- [L] Out-of-office with scheduling
- [L] Undo send (30–60s delay window)
- [L] Send later / scheduled send
- [L] Snooze ("return this thread Monday 09:00")
- [L] Full-text search that is actually fast (store-level index)
- [L] Spam/phishing filtering with a visible "why was this flagged" banner
- [L] One-click unsubscribe surfacing (RFC 8058)
- [2] ★ Follow-up nudges — "no reply after 3 days" resurfacing, per-thread
- [2] ★ Shared-inbox collaboration: assign a thread to a colleague, internal comments on a thread, collision alert ("Kevin is already replying") — Front-style teamwork on info@/sales@ boxes, which Outlook simply cannot do
- [2] Templates / snippets with variables
- [2] Read/delivery status for internal mail (never tracking pixels — privacy is the brand)
- [3] Email client keyboard-shortcut parity (Gmail-style j/k culture)
- [3] S/MIME and OpenPGP for the customers who ask

## Outlook toolbar audit — keep / do-better / drop

Outlook's toolbar is a thin core of email actions wrapped in a ring of Microsoft-ecosystem hooks and third-party add-ins. Almost everything we **drop** below is one of those hooks or add-ins — not email — so Ficina's mail toolbar ends up *cleaner* than Outlook's (the daily actions minus the clutter) **plus** the AI actions Outlook lacks (summarize, draft, why-flagged).

**Keep** — core mail actions, table stakes:

| Outlook | Ficina |
|---|---|
| Reply · Reply All · Forward | [L] the spine of email — identical |
| Delete · Archive | [L] daily one-keystroke actions |
| New Email / New Items | [L] compose |
| Move | [L] into folders |
| Flag · Categories / Tags | [L] flags + color categories, multiple per message, filterable |
| Report junk/phishing | [L] ★ feeds the visible "why was this flagged" banner |
| Quick Steps | [2] one-click multi-action macros (the power-user retention feature) |
| Rules | [L] ★ server-side Sieve — stronger than Outlook's client-side rules, runs even when you're offline |
| Address Book · Search People | [L] from CardDAV contacts |
| Filter Email | [L] sort/filter the list |

**Do better** — keep the capability; Ficina's version is superior:

| Outlook | Ficina |
|---|---|
| New Meeting · Scheduling Poll | [L] Ficina Meet + [2] ★ native meeting polls (kills the Doodle bolt-on) |
| Translate | [2] ★★ AI-native and EU-hosted — the Belgium differentiator, not an add-in |
| Read Aloud | [3] accessibility, later tier |
| Recall | [2] ★ actually works inside a tenant — we own the store, unlike Exchange's famously fake recall |

**Drop** — Microsoft ecosystem tentacles and third-party add-ins; dropping them *removes lock-in*, which is the pitch, not a lost feature:

| Outlook | Why it's gone |
|---|---|
| Share to Teams | replaced by "share to Ficina Chat" |
| Viva Insights | a Microsoft analytics add-in, not our product |
| TeamViewer | a third-party add-in, never a mail feature |
| Browse Groups | an M365 Groups construct — our distribution lists + shared mailboxes cover the real need |
| All Apps | the Microsoft app-grid launcher, irrelevant to a focused workspace |

This table doubles as the answer to a prospect asking "where's feature X?" — every row is kept, done better, or deliberately dropped to cut a lock-in tentacle.

## Agenda

- [L] Personal + shared calendars, free/busy, invitations (iTIP/iMIP)
- [L] Recurring events with exceptions (the interop minefield — done right)
- [L] Room and resource booking
- [L] Working hours, time-zone sanity for cross-border teams
- [2] ★ Booking pages — public "book a slot with me" links (kills the separate Calendly subscription; M365 hides this in Bookings and does it badly)
- [2] ★ Meeting polls — "which of these three slots works?" (kills the Doodle subscription)
- [2] Travel-time blocking between physical meetings
- [3] Team scheduling: round-robin and collective availability for sales/support teams

## Chat

- [L] Channels (public/private), DMs, real threads, reactions, mentions
- [L] File sharing into Drive (one storage, not a parallel one — SharePoint's original sin)
- [L] Powerful search across full history — ★ no paywalled memory, ever (Slack's most-hated limit)
- [L] Guest access for externals, per-channel
- [2] Reminders ("remind me about this message tomorrow"), saved items
- [2] Instant huddle — one-click voice in a channel, no calendar event
- [2] ★ Cross-org channels between two Ficina tenants (agencies ↔ clients)
- [3] Message workflows/automations (approval emoji triggers, simple bots)

## Meet

- [L] Scheduled + instant meetings, calendar-native links, screen share, lobby
- [L] Recording to Drive (with consent indicators)
- [2] ★ AI minutes: transcript, summary, decisions, and action items posted to the meeting's chat thread — included, not a €30/user add-on
- [2] Live captions
- [3] ★ Live translated captions — a Flemish/Walloon/German meeting where everyone reads their own language; the most European feature possible
- [3] Webinar mode (one-to-many, registration)

## Drive & Docs

- [L] Files/folders, per-user and per-team spaces, permissions, trash/restore
- [L] Desktop sync client (the OneDrive replacement)
- [L] Share links with password, expiry, and download-off option
- [L] In-browser editing of Word/Excel/PowerPoint formats (Collabora embedded, Ficina-themed). What users get per editor:

  **Ficina Docs (Word-like)**
  - [L] Full .docx/.odt editing: styles, headers/footers, tables, images, TOC, footnotes, page numbering
  - [L] Real-time co-editing with visible cursors
  - [L] Track changes and comments — round-trippable with desktop Word (the lawyer/HR dealbreaker)
  - [L] Export to PDF; print-faithful layout
  - [2] Compare documents; org templates with locked branding

  **Ficina Sheets (Excel-like)**
  - [L] Full .xlsx/.ods editing: the formula set (LibreOffice Calc covers the overwhelming majority of Excel functions), multi-sheet, cell formatting, conditional formatting
  - [L] Charts, pivot tables, sorting/filtering, freeze panes
  - [L] Co-editing; comments
  - [2] CSV import wizardry; named ranges; data validation
  - Known honest limit: **VBA macros do not run** — flagged by the Migrate audit; the playbook answer is desktop LibreOffice/one perpetual Excel license for the macro workbook (see product doc §6)

  **Ficina Slides (PowerPoint-like)**
  - [L] Full .pptx/.odp editing: layouts, master slides, transitions, presenter notes
  - [L] Present directly in the browser; present into a Meet call
  - [2] Org slide templates; export to PDF handout

- [L] Format fidelity guarantee: files round-trip to desktop Office without layout mangling — tested in CI with a corpus of real customer documents (fidelity is the whole ballgame; a mangled offer letter loses the customer)
- [L] Editors in the desktop app: Docs/Sheets/Slides work identically in the installed (Tauri) app — same frontend, no extra build
- [L] Offline story: synced files open in local LibreOffice/Office while disconnected, changes sync back on reconnect (same model as OneDrive + desktop Office)
- [3] True offline in-app editing (bundled editor engine) — only if customer demand proves it
- [L] Version history with restore
- [2] Document templates (org-branded letter, offer, invoice skeletons)
- [2] Full-text + ★ semantic search inside file contents ("the pdf about the Antwerp lease")
- [3] E-signature workflow (eIDAS-aware — European advantage)
- [3] Retention policies and legal hold per space

## Ficina AI (the differentiator layer — every item ★)

- [L] Semantic search across mail, chat, files in one query bar
- [L] Thread summarization ("catch me up on this 40-mail thread")
- [L] Drafted replies in the user's own tone, user-invoked
- [L] Attachment understanding — incoming .docx/.xlsx read, summarized, figures extractable
- [L] "Where did X go?" migration assistant (change management as a feature)
- [2] Daily digest: "what did I miss" across mail, chat, meetings — the demo that sells
- [2] Inbox triage: priority surfacing, low-value mail folded away, per-user trainable
- [2] ★★ Auto-translation of mail and chat — read and reply across NL/FR/DE/EN transparently; for Belgian and cross-border SMEs this alone justifies switching
- [2] MCP server — customers' AI agents read/search/send under per-agent permissions; the "AI-era workspace" claim made concrete
- [3] Cross-suite actions: "summarize this thread and update the offer sheet" — the Copilot-killer, EU-hosted
- [3] Org memory: "what did we decide about the pricing?" answered from three months of channels and mails

## Admin & platform

- [L] Tenant admin: users, groups, domains, quotas, license seats
- [L] ★ Deliverability autopilot: DNS wizard, DKIM rotation, DMARC monitoring, blacklist alerts — self-hosted mail's killer solved in-product
- [L] Audit log, GDPR subject-access export, tenant data export (no lock-in — exit is a feature)
- [L] SSO: Ficina as OIDC/SAML IdP + 2FA enforcement
- [L] Backup status visibility ("last verified restore: date")
- [2] ★ White-label/reseller mode: MSP branding, multi-tenant management console — the channel play productized
- [2] Per-tenant feature flags and AI on/off switches
- [3] DLP-lite: outbound rules ("warn on external send with attachment X")
- [3] Compliance packs: NIS2 evidence exports, processing-record templates

## Ficina Migrate

- [L] Everything in product-doc §6 — audit, identity, mail furniture, calendars, files+permissions, autodiscover, cutover safety, subscription retirement. Migrate is launch-critical and fully specified there; it is listed here so no one forgets it is a *product*, not a script.

---

## Deliberately absent

No tracking pixels, no ad surface, no engagement mechanics, no consumer free tier, no dark-pattern storage nags. Every absence here is a sales argument; see Non-goals in the product doc for the build-side list.
