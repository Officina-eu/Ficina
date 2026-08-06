# ADR 0034 — Per-product agents

Status: accepted. **Extends [ADR 0029](0029-ai-across-the-workspace.md)** (AI across
the workspace) and builds on [ADR 0011](0011-ai-inference-layer.md) (the inference
layer) and [ADR 0023](0023-propose-then-approve-ai-tasks.md) (propose-then-approve).

## Context

alo is AI-native, but today AI shows up as scattered per-feature actions — Mail's
summarize/draft/smart-reply, Tasks' action-item extraction, the Docs agentic
editor — plus one cross-workspace "Ask alo" cited search (ADR 0029). As the
product grows, users need a consistent mental model: **each product has an
assistant that deeply understands that product's data and actions**, not a grab
bag of buttons and one generic search box.

## Decision

**Every alo product/module has a dedicated agent** — a specialized AI assistant
scoped to that product, with a tool set for that product's actions, all under the
same trust and isolation rules. The cross-workspace **"Ask alo" orchestrator**
(ADR 0029) sits above them and routes a request to the right product agent(s),
composing an answer across them.

| Agent | Owns (examples) |
|---|---|
| **Mail agent** | triage, summarize a thread, draft / smart-reply, extract tasks, find & file, "why flagged" |
| **Agenda (Calendar) agent** | find times, schedule, summarize the day/week, prep for a meeting, propose events from mail |
| **Tasks agent** | propose action items, "what's on my plate", prioritise, chase, organise |
| **Docs agent** | write / edit / clean-paste / inline-diff, agent mode (ADR 0031, 0029) |
| **Sheet agent** | formulas from intent, analysis, clean/transform data, chart-from-intent |
| **Drive agent** | find & organise files, summarise a document, extract from attachments |
| **Chat agent(s)** | first-class chat participants — @mentionable, reply/react (features.md → Chat) |
| **"Ask alo" orchestrator** | cross-product search + routing to the agents above (ADR 0029) |

### Inherited, non-negotiable principles

- **Propose-then-approve** — an agent proposes and diffs; the user accepts. It
  never acts silently (ADR 0023). A **Stop** control on any multi-step run.
- **Access-scoped** — an agent only ever sees and touches what the current user
  already can; every read/write stays tenant-scoped. An agent cannot widen access.
- **EU-only inference**, source-cited — per the model/licensing strategy.

## Consequences

- **One framework, many thin agents.** There is a single shared agent framework —
  the `alo-ai` crate (OpenAI-compatible client + SSRF egress guard, ADR 0011), a
  tool registry, and the propose-then-approve UI. Each product agent is a **thin,
  product-scoped tool set + system prompt** over that framework, not a separate
  system. Build the framework once; add an agent per product as that product lands.
- **Consistent surface.** Each module exposes its agent the same way (an in-module
  assistant), and "Ask alo" can call across them — one mental model everywhere.
- **Build order follows each product's own roadmap.** Mail/Tasks/Docs agents exist
  in part today; Agenda/Sheet/Drive/Chat agents follow their modules. This ADR
  sets the *shape*, not a new phase.

## Rejected

- **One monolithic assistant only** — loses per-product depth and the tool
  scoping that keeps actions safe and legible.
- **Silent autonomous agents** — violates the propose-then-approve trust model,
  the core of the product's promise.
