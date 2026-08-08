# alo UX principles — the interface laws

Owner mandate (2026-08-08): alo's interfaces must feel like the best UX the
user has ever met, grounded in interaction psychology — and **no one should
ever need a menu, manual, or tour to know how an alo app works.** These laws
bind every UI built by any hand (loops, Codex, humans). Each law names its
psychological basis and a verification a wave review can actually run.

## The prime law: zero-manual

**If a screen needs explaining, the screen is wrong — and the user must
NEVER need a menu to use an alo tool.** Everything required to do the job
lives on the surface, in sight, in reach; menus exist for depth and rarities,
never for discovery of the core flow. A first-time user must achieve the
screen's core task in under a minute with no help, no menu-hunting, no tour.
Tooltips and docs may *deepen* understanding, never *enable* it.
*The menu test:* remove every menu from the screen in your head — can the
core task still be completed with what remains visible? If not, redesign.
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
   accountant knows (minus SAP's cruelty); Insights → **Power BI / Tableau-class visualization quality — their output,
   never their learning curve**; Chat → Slack/WhatsApp**. Before designing a screen, name
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

12. **The one-click law** — if an action can be done safely in one click,
    making it two is a defect; every click must earn its place. No ceremony:
    no intermediate "landing" screens before common actions, no navigating
    away to do something that can happen in place, no dialog asking for
    information a default could supply (laws 3, 7, 11b). The most-used action
    of every screen is exactly one click away from where the user already is.
    When extra clicks exist to feel "safe" or "organized", they are neither —
    they are friction wearing a costume.
    *Verify:* for each screen, list its three most frequent actions and count
    the clicks from screen-open to done; any count that could be lower with
    a safe design is filed as a defect.

13. **Brilliant with the AI off** *(economics + philosophy)* — AI calls cost
    money per use; navigation, defaults, and design cost nothing per use.
    Therefore the excellence of every alo flow comes FIRST from deterministic
    UX — galleries, pre-built defaults, direct manipulation, one-click paths,
    zero learning curve — and AI is acceleration and delight on top, never
    the only door to a capability and never a crutch propping up a confusing
    screen. If a flow is only usable "because you can ask the AI", the flow
    is wrong. Every AI feature has a first-class manual sibling (template
    gallery beside generate-my-site; tile gallery beside ask-to-chart), and
    the AI-off configuration is a complete, pleasant product — not a degraded
    mode.
    *Verify:* the wave review walks every module with NO AI key configured;
    every core task must be achievable and feel finished. Count model calls
    in routine flows — a routine action that silently spends an AI call is a
    defect.

## The visual scale — how every size gets chosen

Nothing on an alo screen has an arbitrary dimension. Icon sizes, card
paddings, corner radii, shadows, gaps — every value is picked **by role from
the token scale** (`web/src/ds/tokens.css`), which is the single place a
value may be born. The principles that govern the scale:

- **One rhythm.** All spacing and sizing sits on the token grid (the 4px
  family). Two adjacent elements never differ by amounts the grid can't
  express — that sub-pixel-of-chaos feeling in lesser apps comes exactly
  from breaking this.
- **Size encodes importance.** Bigger means more important, always —
  a screen's visual hierarchy must survive the squint test (law 4). Icons
  come in exactly three roles: inline-with-text, in-controls, and
  rail/hero — chosen by role, never by taste per screen.
- **Radius encodes surface class.** Small radius for controls (inputs,
  buttons), medium for cards and panels, large only for floating overlays —
  one class, one radius, never mixed on the same surface kind. Corner
  radius is how users subconsciously read "what kind of thing is this".
- **Elevation is a ladder.** Shadows come only from the token set, one step
  between adjacent layers; a surface never jumps two levels. Depth is
  information, not decoration.
- **Room to breathe.** Cards and panels keep generous minimum padding from
  the space scale; reading text keeps a book-like measure; density is a
  deliberate mode (as in Drive's density control), never an accident.
- **Touch respects fingers.** Interactive targets stay ≥40px regardless of
  how small the glyph inside them is.
- **New values are legislation, not improvisation.** A dimension that the
  scale lacks is added to tokens.css in its own reasoned commit — never
  inlined into a component.

*Verify (mechanical):* grep new UI for hardcoded px/color values outside the
ds — any literal that isn't a token reference is a defect. *Verify (human):*
the wave review compares sibling screens side by side — same roles must look
identical in size, radius, and rhythm across modules.

## Standing constraints

- All copy through the i18n catalog, in the user's language, jargon-free
  (write "web address", not "subdomain", wherever a normal person will read).
- Keyboard reachability and visible focus on all interactive elements;
  contrast per WCAG AA. Accessibility is part of law 1, not a tier-3 item.
- **`docs/design/` holds laws and standing design decisions ONLY.** Audit
  findings, defect lists, and review outcomes are work items — they live in
  `docs/reviews/` (dated files) or the track queues, never beside the rules.
  A law is permanent; a defect is temporary; mixing them erodes both.
- These laws extend CLAUDE.md quality gates: a UI slice is not done when it
  compiles — it is done when a stranger can use it. Wave reviews test the
  laws explicitly and file violations as queue items.
