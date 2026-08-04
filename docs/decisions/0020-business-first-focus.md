# ADR 0020 — Business-first focus; personal email parked indefinitely

Status: accepted (focus decision by the product owner). Amends the
*priority* of ADR 0018 (consumer personal email); does not revert its
architecture or remove what shipped.

## Context

alo is built by a solo founder. The scarcest resource is not code,
capital, or competitors — it is the founder's time and focus. Every
"should we also do X" is the same trap wearing a different coat, and the
same answer applies: pick the one winnable thing and do it completely.

ADR 0018 added a **personal email** product line (self-service addresses
on a platform domain) and its build slices 1–4 shipped; alomails.com runs
personal signup today. That work is done and stays live. The open
question was strategic, not architectural: **should personal (consumer)
email be an active priority, or is the company (B2B) workspace the focus?**

The evidence all points one way:

- **Two products, one person.** Consumer email and business workspace are
  different customers, economics, and go-to-markets. Consumer is a volume
  game (millions of free users, monetize a sliver, heavy support/abuse
  burden) competing against *free Gmail* — the hardest "free" in tech.
  Business is a focus game: fewer customers, each paying real money.
- **Every advantage points at companies.** The wedge (mail + calendar +
  tasks with cross-suite AI glue) is a *business* workflow — a person does
  not need "turn this email into a task assigned to my colleague."
  Customer zero (Axon) is a company. The revenue model (hosting, per-user,
  partners) is B2B. The differentiation (cross-suite AI, sovereignty for
  organisations) is what *businesses* pay for.
- **Businesses pay; consumers mostly do not.** A company pays ~€8/user/mo
  because email is mission-critical and they want it hosted and supported.
  Gmail anchored consumer email at free forever. For a founder who needs
  revenue to survive, this is not close.
- **Even privacy-first consumer players moved toward business.** Proton
  took years and real funding on consumer, then built Proton for Business
  — because that is where the money is.

## Decision

**Focus entirely on companies. Personal email is parked indefinitely —
not rejected, not removed, just off the active path.**

1. **No further build investment in the consumer line.** ADR 0018 slice 5
   (isolated consumer sending identity — the "go-live at scale" step) and
   any consumer-specific features (billing, consumer onboarding polish,
   consumer growth/retention mechanics) are **not on the roadmap**. They
   are "someday, maybe" — revisited only with a written case and business
   traction that makes it obvious, the same bar Non-goals carry.
2. **What shipped stays.** The personal-email surface built under ADR 0018
   (provisioning primitive, `/signup/*`, the public signup page, password
   reset) remains in the codebase and remains live on alomails.com. Real
   personal accounts exist (e.g. the founder's own dogfooding account);
   deprioritised does not mean deleted or broken.
3. **Roadmap and messaging are business-first.** `docs/features.md` and
   the product description treat the company workspace as the product; the
   personal-email item is explicitly marked parked (see below). New scope
   is judged by "does this help a company adopt alo?", starting with Axon.
4. **The door stays open, cheaply.** Because every personal user is its
   own tenant (ADR 0018), the consumer path costs nothing to keep dormant
   and is trivial to prioritise later *if* alo wins business first. We
   walk through the winnable door first; we do not close the other one.

## Consequences

- **Focus.** Product, roadmap, and the founder's hours point at one
  winnable market. The enemy was never Microsoft or Nextcloud — it is
  divided attention; this removes the largest source of it.
- **alomails.com stays a live consumer surface** (dogfooding + the handful
  of real accounts) without being a growth priority. No new consumer
  infrastructure (a second warmed sending-reputation pool at scale,
  consumer moderation/ops tooling) is stood up until there is a case.
- **Reversible.** This is a priority call, not an architecture change.
  If business traction later makes consumer worth it, ADR 0018's design is
  ready and a future ADR can un-park it with revenue on the table.

## Alternatives rejected

- **Do both now** — rejected: a solo founder splitting across two
  products, two customers, two go-to-markets does neither well. This is
  the "build everything at once" trap the whole project is disciplined
  against.
- **Kill personal email outright** — rejected: needless. It is built,
  live, and free to keep dormant; deleting it would throw away sunk work
  and the cheap optionality of one day expanding, for no focus gain that
  "parked" does not already give.
