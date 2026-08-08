# alo UX principles — the interface laws

Owner mandate (2026-08-08): alo's interfaces must feel like the best UX the
user has ever met, grounded in interaction psychology — and **no one should
ever need a menu, manual, or tour to know how an alo app works.** These laws
bind every UI built by any hand (loops, Codex, humans). Each law names its
psychological basis and a verification a wave review can actually run.

## The prime law: zero-manual

**If a screen needs explaining, the screen is wrong.** A first-time user must
achieve the screen's core task in under a minute with no help. Tooltips and
docs may *deepen* understanding, never *enable* it.
*Verify:* the wave review walks each new screen as a stranger; any step that
required knowing-in-advance is filed as a defect, like S1.30b/c were.

## Ten laws that implement it

1. **Recognition over recall** *(Nielsen heuristic)* — every available action
   is visible where it is used; nothing important lives only in a context
   menu, keyboard shortcut, or memory. Menus may duplicate, never gatekeep.
   *Verify:* core tasks completable with visible controls alone.
2. **Meet expectations users already own** *(Jakob's law)* — people arrive
   trained by the best-known tools of EACH domain, and every alo module must
   match the reflexes of ITS OWN world: Mail/Agenda → Outlook & Gmail; Sheets
   → Excel; Docs → Word & Google Docs; **Sites → Wix/Squarespace-class
   builders; CRM pipelines → Trello-style boards; Billing/ERP → the flows an
   accountant knows (minus SAP's cruelty); Insights → the chart grammar every
   dashboard taught; Chat → Slack/WhatsApp**. Before designing a screen, name
   its domain references and match their reflexes — then beat them on clarity.
   Innovate in what the product does, not in how controls behave.
   *Verify:* each new screen's design/journal names its domain references; no
   control behaves unlike its lookalike in that domain's mainstream tools.
3. **Few choices per moment** *(Hick's law + progressive disclosure)* — a
   screen presents one obvious next step; advanced options unfold only when
   summoned. Default over decision: settings the user never met must already
   be right (the Insights pre-built overview is the canon example).
   *Verify:* count the decisions a new user must make before value — target
   ≤1 per screen.
4. **The primary action is unmissable** *(Fitts's law)* — biggest, closest,
   highest-contrast thing in reach; destructive actions are farther and
   quieter. Touch targets ≥40px.
   *Verify:* squint test — the blurred screen still shows what to press.
5. **Empty states are the onboarding** — a new module's first screen is not
   blank; it teaches the ONE next step and often does it for the user
   (auto-created Home page; pre-built dashboard). Every empty list explains
   itself in one sentence + one button.
   *Verify:* every empty state has action + explanation; none says just
   "No items".
6. **Immediate, honest feedback** *(Doherty threshold, <400ms)* — every click
   visibly responds instantly: optimistic updates, skeletons over spinners,
   progress with meaning. Silence after a click is a defect.
   *Verify:* no interaction leaves the screen unchanged >400ms without a
   working indicator.
7. **Undo over confirmation** *(error tolerance; peak-end)* — routine actions
   execute immediately with a visible undo window; confirmations are reserved
   for the genuinely irreversible/outward (send, issue, publish, delete-forever
   — where alo's propose-then-approve pattern rules). Never punish the 99%
   flow to guard the 1% mistake.
   *Verify:* count confirm dialogs per module; each must justify itself
   against this law.
8. **Errors speak human and help** — say what happened, why, and the way out,
   in the user's words; surface the server's precise reason verbatim rather
   than a generic veil (the S1.30b lesson, now law). No codes, no blame.
   *Verify:* trigger each error path; every message names a next step.
9. **Calm surfaces, one voice** *(aesthetic-usability; Miller's law)* — the
   token system is the single source of color/type/spacing; one accent per
   surface; information chunked in scannable groups; motion subtle and
   purposeful (reduced-motion respected). Beauty here is trust, not
   decoration.
   *Verify:* no hardcoded colors/spacings outside tokens; screens pass a
   5-second "what is this page about" glance test.
10. **End on a high** *(peak-end rule)* — completing something meaningful
    (site published, invoice issued, week approved) earns a small, fast
    moment of acknowledgment with the result's identity (the live URL, the
    number assigned) — never a modal essay.
    *Verify:* each module's "done" moments show the outcome, not just close.

11. **Smoother than the tool they came from** *(flow: the goal-gradient
    effect, and the reason to switch)* — familiarity is the floor; the flow
    is where alo wins. Three measurable rules:
    (a) **Step budget:** every core journey takes FEWER clicks/screens than
    the same journey in its domain-reference tool — count them both in the
    design journal, and beat the reference or justify why not.
    (b) **Never ask what alo already knows:** data entered once flows
    everywhere — a customer typed in CRM autofills the invoice, the site
    form, the email; a user should never re-type or copy-paste anything
    between alo modules, ever.
    (c) **No seams:** the flows competitors need exports/integrations for
    are ONE motion here (won deal → invoice; site form → CRM lead; hours →
    invoice line; question → chart on the board). alo owns the whole house —
    walking between rooms must feel like one floor, not doorways with locks.
    *Verify:* wave reviews map each core journey's step count vs. the
    reference tool, hunt for any re-typed field, and walk every cross-module
    handoff end-to-end.

## Standing constraints

- All copy through the i18n catalog, in the user's language, jargon-free
  (write "web address", not "subdomain", wherever a normal person will read).
- Keyboard reachability and visible focus on all interactive elements;
  contrast per WCAG AA. Accessibility is part of law 1, not a tier-3 item.
- These laws extend CLAUDE.md quality gates: a UI slice is not done when it
  compiles — it is done when a stranger can use it. Wave reviews test the
  laws explicitly and file violations as queue items.
