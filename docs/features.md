# alo — features.md

Feature inventory per module. Three tiers, mapped to the roadmap:
**[L]** = launch (must exist to cancel M365) · **[2]** = fast-follow, first year after launch · **[3]** = later, revenue-funded.
**★** marks differentiators — features Microsoft either lacks, charges extra for, or does badly.

Rule of the file: nothing gets built that isn't listed here, and nothing gets listed without a tier. Additions go through the scope gate (product doc, Non-goals).

---

## AI — an agent for every product (ADR 0034)

Cross-cutting principle: **every product has its own dedicated agent**, scoped to
that product's data + actions, all under **propose-then-approve** (never silent),
**access-scoped** (only what the user can already see/do), and **EU-only** models.
Above them sits the **"Ask alo" orchestrator** (ADR 0029) that routes across
products. One shared framework (the `alo-ai` crate + a tool registry + the
propose/approve UI); each agent is a thin, product-scoped tool set + prompt.

- [L] ★ **Mail agent** — triage, summarize a thread, draft / smart-reply, extract tasks, "why flagged" *(largely built)*
- [L] ★ **Tasks agent** — propose action items, "what's on my plate", prioritise *(built, ADR 0023)*
- [L] ★ **Docs agent** — write / edit / clean-paste / inline-diff, agent mode *(built, ADR 0029/0031)*
- [2] ★ **"Ask alo" — the top-level agent** — not just search: a workspace-wide agent you ask in plain language to **answer AND act** across products ("summarise the Acme thread and draft a reply", "block two hours tomorrow and email the team"), running multi-step tasks by orchestrating the product agents below — cited, propose-then-approve, access-scoped. The universal command bar for the whole workspace *(cross-product cited search + doc AI built today; acting/orchestration is the growth)*
- [2] ★ **Agenda (Calendar) agent** — find times, schedule, summarize the day/week, prep a meeting, propose events from mail
- [2] ★ **Sheet agent** — formulas from intent, analysis, clean/transform data, chart-from-intent
- [2] ★ **Drive agent** — find & organise files, summarize a document, extract from attachments
- [2] ★ **Chat agent(s)** — first-class chat participants, @mentionable, reply/react (see → Chat)
- [3] Per-agent skills users can create and share; the browseable agent directory

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
- [L] Large files as expiring share links (alo Transfer) — a file too big to
  attach uploads once and rides the message as a private, expiring download link
  instead of an inline attachment, sidestepping recipient attachment-size limits.
  This is the Drive share-link capability (password/expiry/download-off) surfaced
  in compose; v1 ships an unguessable-link + expiry, with password/download-off
  tracked to the Drive work.
- [2] ★ Follow-up nudges — "no reply after 3 days" resurfacing, per-thread
- [2] ★ Shared-inbox collaboration: assign a thread to a colleague, internal comments on a thread, collision alert ("Kevin is already replying") — Front-style teamwork on info@/sales@ boxes, which Outlook simply cannot do
- [2] Templates / snippets with variables
- [2] Read/delivery status for internal mail (never tracking pixels — privacy is the brand)
- [3] Email client keyboard-shortcut parity (Gmail-style j/k culture)
- [3] S/MIME and OpenPGP for the customers who ask

## Outlook toolbar audit — keep / do-better / drop

Outlook's toolbar is a thin core of email actions wrapped in a ring of Microsoft-ecosystem hooks and third-party add-ins. Almost everything we **drop** below is one of those hooks or add-ins — not email — so alo's mail toolbar ends up *cleaner* than Outlook's (the daily actions minus the clutter) **plus** the AI actions Outlook lacks (summarize, draft, why-flagged).

**Keep** — core mail actions, table stakes:

| Outlook | alo |
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

**Do better** — keep the capability; alo's version is superior:

| Outlook | alo |
|---|---|
| New Meeting · Scheduling Poll | [L] alo Meet + [2] ★ native meeting polls (kills the Doodle bolt-on) |
| Translate | [2] ★★ AI-native and EU-hosted — the Belgium differentiator, not an add-in |
| Read Aloud | [3] accessibility, later tier |
| Recall | [2] ★ actually works inside a tenant — we own the store, unlike Exchange's famously fake recall |

**Drop** — Microsoft ecosystem tentacles and third-party add-ins; dropping them *removes lock-in*, which is the pitch, not a lost feature:

| Outlook | Why it's gone |
|---|---|
| Share to Teams | replaced by "share to alo Chat" |
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

## Tasks

The third leg of the mail + calendar + tasks wedge — one record, board and
list views over the same data, personal and team. ADRs 0021–0023.

- [L] Tasks: title, description, assignee, due date, priority, subtasks, comments, activity history, attachments — one record, tenant-scoped
- [L] ★ Board (kanban) and list are two views of the SAME task — instant, lossless switch; drag to move between columns / reorder (ADR 0022)
- [L] Personal tasks (private) and team tasks (shared projects with assignees) — one data model, different scoping (ADR 0021)
- [L] ★ Task detail as a slide-in side panel (never navigates away): description, subtasks, comments, activity, source link
- [L] ★ Source link: a task remembers the email or calendar event it was created from, and can jump back to it (email→task, meeting→task)
- [L] ★ AI proposes action items from a meeting/email; the user accepts the real ones — propose-then-approve, never silent creation (ADR 0023)
- [L] Task ↔ calendar: a task with a due date surfaces on the calendar alongside events
- [L] ★ "What's on my plate today" — an aggregate the AI assembles from tasks (+ calendar + mail as they connect)
- [2] Per-project membership + roles for team projects (v1 scopes team projects tenant-wide)
- [2] Recurring tasks, task dependencies, custom board columns
- [3] Workload view, timeline/Gantt, task templates

## Chat

- [L] Channels (public/private), DMs, real threads, reactions, mentions
- [L] **Rich, modern chat UI** — the visual bar is Slack/Teams-grade, not a bolt-on. Design reference: **Sila (silahq.com)**: a left sidebar (DMs, channels, agents, shared, search), a clean message feed with human avatars + distinct agent icons, message bubbles with sender + timestamp, inline media/link previews, hover actions (react/reply/more), typing + presence indicators, unread badges.
- [L] File sharing into Drive (one storage, not a parallel one — SharePoint's original sin)
- [L] Powerful search across full history — ★ no paywalled memory, ever (Slack's most-hated limit)
- [L] Guest access for externals, per-channel
- [2] ★ **Agent-native chat** (the AI-native differentiator, à la Sila) — the per-product agents (ADR 0034) are first-class participants in channels/DMs with their own avatars/indicators. @mention an agent and it **talks back in-thread AND takes actions in its product**: the Mail agent drafts/sends, the Sheet agent updates a range, the Agenda agent books a slot, the Docs agent edits — every *action* still **proposed then approved** (never silent), **access-scoped** to the asking user (even in shared/cross-org channels), cited/auditable. Chat becomes the shared human+agent command surface. Browseable agent directory with usage. EU-only inference.
- [2] ★ AI/natural-language search **and** notifications — "notify me when the Acme deal is mentioned", "where did we decide the price?" — over full history, no paywalled memory.
- [2] Expiring messages and time-limited groupchats (ephemeral conversations that clean themselves up), à la Sila.
- [2] Reminders ("remind me about this message tomorrow"), saved items
- [2] Instant huddle — one-click voice in a channel, no calendar event; ★ with call transcription + auto notes posted back to the thread
- [2] ★ Cross-org channels between two alo tenants (agencies ↔ clients) — incl. shared human **and agent** coordination across tenants
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
- [L] **Native editors — alo's own UI on embedded open engines** (ADR 0033, replacing the earlier Collabora-embedded approach). Real Office files import best-effort into the native types; the original file is kept and stays downloadable; pixel-faithful round-trip to desktop Office is no longer promised. Per editor:

  **alo Docs (Word-like)** — alo's own block editor on **BlockNote** (MPL-2.0)
  - [L] Rich text: styles, headings, lists, tables, images, code blocks, math equations (KaTeX)
  - [L] Propose-then-approve document AI (ADR 0029)
  - [L] Open a real `.docx` → best-effort import into an alo Doc
  - [2] Real-time co-editing; comments; export to PDF

  **alo Sheets (Excel-like)** — alo's own ribbon UI on **Univer** (Apache-2.0)
  - [L] Grid + formula engine, multi-sheet, cell formatting, number formats, alignment, merge, freeze panes
  - [L] Open a real `.xlsx` → best-effort import; **export any sheet back to `.xlsx`**
  - [2] Charts, pivot tables, sorting/filtering, data validation (Univer plugins, wired incrementally)
  - Known honest limits: **VBA macros do not run**; complex `.xlsx` styling/charts may not survive import (see product doc §6)

  **alo Slides (PowerPoint-like)** — native canvas built in-house (no open engine covers it; ADR 0033)
  - [2] Slides, text boxes, shapes, images; best-effort `.pptx` import
  - [2] Present directly in the browser; present into a Meet call

- [L] Format fidelity guarantee: files round-trip to desktop Office without layout mangling — tested in CI with a corpus of real customer documents (fidelity is the whole ballgame; a mangled offer letter loses the customer)
- [L] Editors in the desktop app: Docs/Sheets/Slides work identically in the installed (Tauri) app — same frontend, no extra build
- [L] Offline story: synced files open in local LibreOffice/Office while disconnected, changes sync back on reconnect (same model as OneDrive + desktop Office)
- [3] True offline in-app editing (bundled editor engine) — only if customer demand proves it
- [L] Version history with restore
- [2] Document templates (org-branded letter, offer, invoice skeletons)
- [2] Full-text + ★ semantic search inside file contents ("the pdf about the Antwerp lease")
- [3] E-signature workflow (eIDAS-aware — European advantage)
- [3] Retention policies and legal hold per space

## alo Docs — the AI-native document editor

Not a cheaper European Word. alo Docs differentiates on being **AI-native,
whole-suite, and sovereign**, attacking documented, widespread Word/Docs
frustrations that Microsoft/Google structurally cannot fix without dismantling
their own architecture. The editor is **alo's own block editor on the embedded
BlockNote framework** (MPL-2.0); alo owns the UI, the AI layer, and the four
inventions below (ADR 0033, superseding the Collabora shell of ADR 0010). Base
.docx import lives under
**Drive & Docs** above; this is the differentiator layer. UX source of truth:
Figma page "10 · Docs".

The four inventions:

- [2] ★ **Clean paste** — on paste from external sources (Word, the web), strip
  foreign formatting **by default** and match the destination document's
  styles; show a dismissible toast ("Pasted from <source> — formatting
  cleaned") with a "Keep original" escape hatch. Targets the #1 documented
  Word/Docs pain: foreign styles corrupting a document on paste.
- [2] ★ **Ask-AI-from-your-docs** — an in-editor AI panel that answers from the
  user's *actual* documents and workspace, not just the open doc ("What did we
  offer Proceq last quarter?" → pulls the real file from Drive). Every answer
  carries a **source citation** (which file it came from); cross-suite (Mail,
  Drive, Calendar); with suggested actions ("insert into the doc", "summarize
  this section"). It is **agentic, not just Q&A** — see below.
- [3] ★ **Semantic-conflict flag** — beyond CRDT text-merge: when two
  collaborators' edits no longer reconcile in *meaning* (one changes a unit
  price, another the total, so they no longer add up), the AI surfaces an
  inline flag ("alo noticed a possible conflict — these no longer add up")
  with keep-A / keep-B / let-me-fix. Directly targets the documented
  silent-corruption of Word/Docs real-time co-authoring, which merges
  conflicting edits into nonsense with no warning.
- [3] ★ **Draft-from-workspace-context** — on a new/empty doc, offer to draft it
  from real workspace context: the AI lists the sources it will use (the
  relevant email thread, a meeting recording + its AI notes, related
  spreadsheets) and generates a first draft from them. The cross-suite killer
  move — only possible because alo owns Mail + Meet + Drive + Docs in one
  sovereign place.

**Ask-AI is agentic** — it acts on the document, always **proposing, never
silently changing**:

- [2] ★ Inline command: select text → a command bar (Rewrite / Shorten / Fix
  grammar / custom instruction).
- [2] ★ Proposed edit: AI changes are shown as an inline **diff** (old struck
  through, new highlighted) with **Accept / Reject** — nothing applies without
  approval.
- [3] ★ Agent mode: multi-step tasks ("add a delivery-terms section and tighten
  the intro") execute as a visible **plan** with per-step status
  (done/doing/pending), a live progress note, workspace-context grounding, and
  a **Stop** control; the doc shows where the AI is actively writing.
- **Core principle:** the AI proposes and diffs; the user accepts. It never
  overwrites the document without explicit approval — the trust model that fits
  a sovereignty product.

**Technical authoring** — specs with math, equations, and code, for engineers,
finance, and technical writers. A alo-owned shell capability (ADR 0015)
rendered **browser-local** (no draft equation or source line leaves the client);
KaTeX + Prism (both MIT); the numbering/reference layer is alo's own. UX
source of truth: the Figma technical-authoring screens.

- [2] ★ **Equations** — an equation editor with LaTeX input and a **live
  rendered preview** (KaTeX), a LaTeX/Visual toggle, and a common-symbols quick
  bar; supports both **inline math** (within a sentence) and **numbered display
  equations**.
- [2] ★ **Code blocks** — syntax-highlighted code (Prism) with a **searchable
  language picker** (explicit, never auto-detected), a copy button, and line
  numbers.
- [2] ★ **Cross-references + auto-numbering** — equations, tables, figures, and
  sections get **auto-numbers**, and reference chips ("Eq. 3", "Table 1",
  "Section 2.3") **stay correct automatically** when items are reordered or
  inserted (references point at an item's identity, resolved to its current
  number). Includes the insert-cross-reference picker (tabs for Equations /
  Sections / Tables / Figures).

Cross-cutting Docs principles:

- [L] No hidden formatting: visible structure, an always-available "clean
  formatting", and block-safe editing that can't be accidentally broken —
  while preserving a **print-perfect "paper" view** for formal documents
  (offers, contracts).
- [L] Version confidence: persistent plain-language save/version state ("Saved ·
  v14 · Kevin edited 2 min ago") with a human-readable timeline.
- [L] ★ Web-first, single-version — no desktop-vs-browser split (the one thing
  everyone praises Google Docs for).

## alo Sheets — the AI-native, auditable spreadsheet

Not a cheaper European Excel. Differentiates on **AI-native + auditable +
whole-suite + sovereign**. Finance teams abandon spreadsheets over two things
the research documents clearly: **error-blindness** (a CFO study found 41%
struggle to identify and correct errors) and **lack of auditability / data
lineage**. alo attacks both directly. The editor is **alo's own ribbon UI on
the embedded Univer engine** (Apache-2.0), the same pattern as Docs (ADR 0033,
superseding ADR 0010); alo owns the UI, the AI layer, and the inventions below.
Base .xlsx import + `.xlsx` export lives under **Drive & Docs** above. UX source of truth:
Figma page "11 · Sheets".

The four inventions:

- [2] ★ **Explain-and-fix errors** — replace cryptic #REF!/#VALUE!/#NAME? with a
  plain-language card: *why* it broke ("row 14, referenced by D5, was deleted")
  plus one-click fixes (re-point the range / restore the row). AI proposes, user
  accepts.
- [2] ★ **Natural-language formulas** — type plain English ("average revenue per
  region, excluding France"); alo generates the formula, **shows the actual
  formula**, and explains it in one line. Never a black box — transparent and
  auditable (treat NL as a draft, keep the transparent formula).
- [2] ★ **Formula paste-guard** — when a raw value is about to overwrite a
  formula cell, warn ("E5 holds =SUM(D2:D13) — paste as value anyway, or keep
  the formula?"). Defends against the documented "pasted value silently ruined
  my model" failure that Excel has no guard for.
- [3] ★ **Ask-your-data** — an NL question panel ("which region is trending
  down?") → an answer with the **source cells cited**, the cells highlighted,
  and a chart. Cross-suite (can pull from Drive/Mail). Every answer traceable
  to its cells.

Cross-cutting Sheets principles:

- [2] ★ Auditability first: cell lineage ("where did this number come from + who
  changed it"); answers and AI edits always cite their source cells.
- [L] AI proposes, user accepts — never silently changes a value or a formula
  (the trust model shared with Docs; critical for audit-ready finance models).
- [L] Cross-platform migration safety: handle Excel-dialect formulas
  (semicolon/comma) on import so formulas don't break moving in.
- [3] Optional agent mode for multi-step data tasks ("build a Q3 forecast from
  the actuals tab") with a visible plan + approval, mirroring the Docs agent.

## alo AI (the differentiator layer — every item ★)

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
  - [L] Domain verify + record guidance, registrar-universal (read-only DNS checks; works at any registrar) — done, ADR 0012
  - [2] Per-tenant DKIM keys with selector-rollover rotation — ADR 0014
  - [3] ★ "Just works" onboarding: change your nameservers, we run authoritative DNS and manage the whole zone (MX/SPF/DKIM/DMARC/mta-sts) automatically — the universal, sovereign path (not per-registrar APIs). DKIM-CNAME makes rotation no-touch on top of it. Engine + direction: ADR 0013
- [L] Audit log, GDPR subject-access export, tenant data export (no lock-in — exit is a feature)
- [L] SSO: alo as OIDC/SAML IdP + 2FA enforcement
- [L] Backup status visibility ("last verified restore: date")
- [2] ★ White-label/reseller mode: MSP branding, multi-tenant management console — the channel play productized
- [2] Per-tenant feature flags and AI on/off switches
- [parked] Personal email (self-service): individuals self-register an address (e.g. `johnsmith@alomails.com`) on a platform-operated domain — one tenant per person, verification-gated signup, consumer sending reputation isolated from B2B. **Parked indefinitely — off the active path (ADR 0020): the focus is the business workspace.** What shipped under ADR 0018 (provisioning, `/signup/*`, the signup page, password reset) stays live on alomails.com for dogfooding + existing accounts, but no further consumer investment (ADR 0018 slice 5 / consumer growth) is on the roadmap. "Someday, maybe" — un-parked only with a written case and business traction — ADR 0018, ADR 0020
- [3] DLP-lite: outbound rules ("warn on external send with attachment X")
- [3] Compliance packs: NIS2 evidence exports, processing-record templates

## alo Migrate

- [L] Everything in product-doc §6 — audit, identity, mail furniture, calendars, files+permissions, autodiscover, cutover safety, subscription retirement. Migrate is launch-critical and fully specified there; it is listed here so no one forgets it is a *product*, not a script.

---

## Deliberately absent

No tracking pixels, no ad surface, no engagement mechanics, no consumer free tier, no dark-pattern storage nags. Every absence here is a sales argument; see Non-goals in the product doc for the build-side list.
