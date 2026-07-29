# ADR 0009 — Remote desktop control: integrate a self-hostable engine, don't build one

**Status:** accepted · 2026-07

**Decision:** Ficina's **Meet** module will offer remote desktop control
(screen takeover) — but Ficina will **not build a remote-control engine from
scratch**. It will **integrate a self-hostable engine**; the primary
candidate is **RustDesk** (open-source, Rust, self-hostable relay,
EU-deployable, aligned with our stack). Ficina owns the **UI/UX, session
brokering, authentication, consent, and audit logging**; the integrated
engine owns the **screen capture, stream, and input injection**. This is a
specific application of ADR 0003 (engines are integrated, not built) to the
single most dangerous capability in the product.

Remote control is an **expansion beyond the M365-replacement core** —
Microsoft 365 has no equivalent — so it is sequenced **late** (post-launch,
ROADMAP Phase 6), after the core suite ships. Screen **share** (read-only,
already in Meet) is in-category and built; remote **control** is a separate,
later, integration-based capability.

**Why:** Remote control is the **highest-risk security surface in enterprise
software**. The engines that dominate the European IT-management market —
AnyDesk (Stuttgart) and TeamViewer (Göppingen) — are exactly the kind of tool
attackers target: AnyDesk disclosed a production-systems breach in 2024, and
TeamViewer is a recurring vector in ransomware intrusions. Building an
input-injection/OS-control engine, unaudited, into a **young sovereignty
product whose entire value proposition is trust** would be an unacceptable,
existential risk — one CVE in a home-grown capture-and-control stack could
end the company. Integrating a mature, auditable, open-source engine and
owning only the trust-bearing layers (consent, audit, brokering) puts our
scarce security attention where our differentiation actually is. The
market pitch is real: **one sovereign EU suite instead of stitching a
separate remote tool** onto a workspace — but only if we can make the trust
claim honestly, which building-from-scratch would forfeit.

**Requirements the integration must satisfy** (non-negotiable, from the
security posture this ADR exists to protect):

- **Native agent per controlled device.** Browsers cannot grant OS-level
  input/screen control — this is a hard technical and security boundary. A
  controlled machine runs a native Ficina/RustDesk agent; there is no
  "control from a tab" path.
- **End-to-end encrypted session**, keys not held by any relay.
- **Explicit per-session consent** by the controlled user *before any input
  is accepted* — no standing/unattended control in v1.
- **A full audit-log entry** in the controlled user's own security log for
  every session (who, when, from where, duration).
- **Instant termination by either party**, always, with a persistent
  active-control banner on the controlled screen.
- **Self-hosted relay** — no third-party cloud in the path — to preserve the
  sovereignty guarantee. The relay is a pinned, self-hostable component, run
  like our other engines.

**Launch surfaces** (recorded in ROADMAP): **primary — Chat** (the 1:1 DM
header and the person-profile quick-actions, beside Meet/Call/Email), because
IT-support "help me" conversations live in chat and that is the
low-friction entry; **secondary — Meet** (an in-call control-bar button for
the "take over while we're already talking" case). A dedicated
**Remote/Support rail tab is deferred** to a later stage — added only once
the feature is mature enough to need its own session management, history, and
audit views.

**Rejected — build the capture/stream/input-injection engine ourselves.**
It would be a multi-quarter effort in the highest-CVE-density domain in the
product, defended by a two-person team, with no differentiation to show for
it (the user cannot tell whose pixels are streaming). The failure mode is
catastrophic and brand-ending. Integration lets us ship the capability with
the trust story intact and our review budget spent on consent/audit/brokering
— the parts a customer actually evaluates us on.

**Rejected — ship remote control early, alongside screen share.** Screen
share is read-only and in-category; remote control is input-injection and
out-of-category (M365 has no equivalent). Bundling them would rush the most
dangerous surface into the product before the core suite and its security
audit (Phase 5) exist. Correctly sequenced late.

**Rejected — a third-party cloud remote tool (AnyDesk/TeamViewer/embedded
SaaS).** It reintroduces exactly the foreign-dependency and
data-in-someone-else's-cloud problem Ficina exists to remove, on the most
sensitive surface of all. A self-hosted relay is mandatory.

**Consequences:** The **UX contract** (request access, the controlled user's
consent prompt, the active-control banner with stop-sharing) is owned by
Ficina and specified in the Figma design as the UX source of truth; it
changes additively. The engine is a **pinned, integration-tested
component**, version-bumped like Synapse/LiveKit/Collabora (ADR 0003); a
source patch to it requires its own ADR. Remote control **does not appear in
`docs/features.md` as a core-suite feature** — it is a Phase-6 expansion, and
this ADR plus the ROADMAP entry are its record until it enters a build phase.
