# ADR 0011 — AI inference: a model-agnostic layer with a bring-your-own backend

**Status:** accepted · 2026-07

**Decision:** Ficina's AI features are served by **`ficina-ai`**, a
**model-agnostic inference layer** that speaks **one backend shape: the
OpenAI-compatible Chat Completions API** (`{base}/v1/chat/completions`). That
single contract covers every backend we care about — **Ollama** and **vLLM**
(self-hosted, local, no key), **Mistral** and other EU providers, and any
OpenAI-compatible endpoint — so there is no per-provider SDK and no lock-in.
The backend is **configured, never bundled**: an operator points Ficina at a
base URL + model, with an optional API key, and toggles it on. This is the
direct implementation of product-description §13 (AI model strategy) and
ROADMAP Phase 3 ("model-agnostic inference API; self-hosted GPU path;
per-tenant off-switch"); it settles the *how*, which those already fixed as
the *what*.

Configuration is **per-tenant and admin-set** (base URL, model, optional API
key, enabled flag), stored tenant-scoped. `ficina-ai` is a **Rust crate**
(below the waterline, ADR: two languages only) surfaced through the existing
**`ficina-jmap` gateway** — reusing its authentication and account door —
rather than a separate container in v1; it may graduate to a standalone
service if load ever warrants (the crate boundary makes that a lift-and-shift,
not a rewrite). The **first consumer is compose "Improve"** (polish a draft on
request); thread summarization and drafted replies (Phase 3) are later
consumers of the same layer.

AI is **pulled forward from Phase 3 into the Phase 2 tail** via the ROADMAP's
marked ⇄ overlap, because a single user-invoked feature (Improve) exercises
the whole layer end to end and de-risks the rest.

**Why:**

- **The model landscape shifts faster than any other dependency.** Committing
  to one provider's SDK would date the product in months. An interchangeable
  layer behind one wire contract lets the model change without touching a line
  of feature code.
- **Sovereignty is the product.** A sovereign, open-source workspace cannot
  ship a mandatory call to a US API. The default self-hosted path is the
  operator's **own** Ollama/vLLM on their **own** hardware — customer data
  never leaves the tenant. Cloud tenants get an EU-hosted open-weight default
  under DPA. Either way the endpoint is a deployment choice, not a hard-coded
  Anthropic/OpenAI dependency.
- **OpenAI-compatible is the lingua franca.** Ollama, vLLM, and most hosted
  providers already expose `/v1/chat/completions`. One client, correctly
  built, reaches all of them — maximal reach for minimal code, which is the
  only affordable posture for a two-person team.
- **Open source demands it.** Anyone running the AGPL core must be able to run
  the AI layer against a model they control, with nothing to purchase. BYO
  backend is what makes "open source, verifiable sovereignty" true of the AI
  layer and not just the mail core.

**Requirements the layer must satisfy** (non-negotiable):

- **Per-tenant configuration and a per-tenant off-switch.** AI is off until an
  operator configures and enables it; a tenant with it disabled behaves as if
  the features do not exist (the "Improve" affordance is hidden, never a dead
  button).
- **Admin-set endpoint, not arbitrary per-user URLs.** The server makes an
  outbound request to the configured base URL; letting any user set that URL
  is an SSRF vector. Endpoint configuration is a tenant-admin capability. (A
  future allowlist/egress policy is noted for the cloud multi-tenant case.)
- **Tenant-scoped throughout.** Config, requests, and responses are read and
  written only within the account door; inference text never crosses a tenant
  boundary (product §13 guarantee).
- **Content is tenant data (law #1).** The only thing sent to the backend is
  the text the user asked us to act on. Message bodies, prompts, and
  completions are **never logged** — not in request logs, not in errors.
- **User-invoked only in v1.** No background/automatic inference; every call
  is a deliberate user action, keeping cost and privacy predictable.
- **Graceful degradation.** Unconfigured, disabled, or unreachable backends
  fail soft with a clear message and **never block the underlying action**
  (you can always still send the draft you wrote).
- **No training on customer data.** A contractual guarantee for hosted; for
  self-hosted it is the operator's own model. We transmit nothing beyond the
  invoked request.

**Rejected — bundle a provider (Anthropic/OpenAI SDK, a specific model).**
Lock-in, a mandatory foreign dependency on the most-scrutinised surface of a
sovereignty product, and obsolete the moment the frontier moves. Fails the
core pitch.

**Rejected — build/serve our own model.** Absurd for a two-person team and no
differentiation; serving is a solved, commoditised problem (Ollama/vLLM). Our
value is the tenant-local *integration*, not the tokens.

**Rejected — a bespoke per-backend protocol abstraction.** Unnecessary: the
OpenAI Chat Completions shape is already the de-facto standard that Ollama and
vLLM implement. One adapter, not N.

**Rejected — per-user endpoint configuration.** Convenient but an SSRF and
support-surface hazard; the endpoint is infrastructure, owned by the operator/
tenant admin, not an end-user preference.

**Rejected — a separate `ficina-ai` service/container in v1.** Premature
operational weight for one user-invoked feature. The crate-behind-the-gateway
form ships the same contract now; the boundary is drawn so a later split is
mechanical.

**Consequences:** A new Rust crate **`ficina-ai`** (inference client + config
types + prompt construction) joins the workspace. A tenant-scoped **AI config**
store (expand migration) holds base URL / model / key / enabled, admin-set. The
gateway gains an additive **`/ai/improve`** route (auth + account door); its
request/response schema is a public contract, versioned additively like the
JMAP surface. A **Settings** surface (Figma is the UX source of truth) lets an
admin configure and toggle the backend. The compose **"Improve"** control is
wired to this route and is shown **only when AI is enabled** for the tenant —
it is never a stub. **End-to-end verification requires a reachable backend**
(the operator's Ollama or an API key); CI and our own tests exercise the layer
against a mock and cover the off/disabled/unreachable paths, and the
plumbing is validated independently of any specific model. This ADR and the
ROADMAP Phase 3 entry are the record until the remaining consumers (summaries,
drafted replies) enter their build.
