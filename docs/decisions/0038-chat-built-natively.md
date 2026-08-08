# 0038 — alo Chat is built natively, not on Matrix/Synapse

Date: 2026-08-08 · Status: accepted · **Supersedes the chat clause of ADR 0003**
(Synapse per tenant); ADR 0003 stands unchanged for LiveKit and Garage.

## Decision

**alo Chat is built on alo's own store**, like Mail, Billing and Sites —
channels, DMs, threads and reactions as tenant-scoped tables, served by
`alo-jmap` under `/chat/*`, delivered live over the **existing RFC 8620
EventSource push stream**, with files stored as **Drive nodes** and history
searched by the **existing workspace index**. Synapse is not deployed.

## Why the July decision no longer fits

ADR 0003 chose Synapse because "each engine is 5–15 years of work with zero
differentiation value". That reasoning holds for LiveKit (real-time media)
and Garage (object storage). It stopped holding for chat, for four reasons:

1. **Matrix's two gifts are unusable here.** Federation with the public
   Matrix network was never a goal ("Matrix invisible", ADR 0003 itself), and
   **E2EE is incompatible with the two starred differentiators**: agents
   participating in channels (ADR 0034) and search across full history —
   both require the server to read messages. We would carry the whole engine
   for the parts we must switch off.
2. **It fights our storage law.** Matrix has its own media repository;
   `features.md` explicitly condemns a parallel file store as "SharePoint's
   original sin". Chat files must be Drive nodes.
3. **The platform now supplies what chat needs** — it did not in July: the
   tenant-scoped store, `alo-identity`, Drive, the workspace search index,
   Spaces permissions, the agent framework, and (decisively) a working
   **push stream the web client already consumes**. Chat becomes tables +
   routes + a UI, not a new runtime.
4. **Per-tenant Synapse is real operational weight** — a second server and
   database per tenant to provision, upgrade, monitor and back up — carried
   by a team whose whole doctrine is "one store, one product, few moving
   parts".

## What we give up, honestly

Public-Matrix federation and end-to-end encryption. Both are recorded as
non-goals for chat v1, not oversights. If a customer ever requires Matrix
interop, a **bridge** can be written against our own API later — interop as
a feature, never as the foundation.

## Consequences

- Message ordering is a **per-channel monotonic sequence** (the pattern the
  mailbox UID and the gapless invoice number already use): clean pagination,
  read state as one integer, idempotent sync.
- Live delivery adds chat types to the existing push hub — **no WebSocket
  layer**, nothing new for Caddy to proxy, one pipe for the whole workspace.
- Agents are ordinary participants under ADR 0034 (propose-then-approve,
  access-scoped to the asking user), not bridge bots.
- Design detail: `docs/design/chat.md`. Build ladder: the Chat track,
  phase by phase, human-supervised (not an autonomous queue).
