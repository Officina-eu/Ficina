# ADR 0002 — AGPL-3.0 core with commercial dual licensing

**Status:** accepted · 2026-07

**Decision:** Ficina's core is open source under AGPL-3.0; commercial
licenses are sold for AGPL-exit; parts of the control plane/billing
may stay proprietary (boundary tracked in Open Decisions). All outside
contributions require a CLA granting relicensing rights — enforced
from the first public commit.

**Why:** Open source is the sovereignty pitch made verifiable; AGPL
forces hosting competitors to publish changes; dual licensing is the
proven funding model of our nearest comparables (Element, grommunio,
Odoo's trajectory). The CLA is what keeps the open/paid boundary OURS
to draw forever.

**Rejected:** permissive (arms hosting competitors), closed source
(kills the trust story), BSL (fails the "is it open source" audit
question our buyers actually ask).

**Consequences:** we may never merge outside code without a signed
CLA; a public source page in the product links our repos and upstream
engines; our own patches to AGPL engines (rare by doctrine) go in a
public patches repo.
