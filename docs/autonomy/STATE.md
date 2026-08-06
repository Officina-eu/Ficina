# STATE.md — loop journal (append-only; newest at the bottom)

The loop appends one entry per iteration: item id, what shipped, how it was
verified, anything cut or flagged for human review, and the next item. Humans
read this file with morning coffee; the loop reads it to regain context.
The end-of-queue / emergency-stop control markers the wrapper watches for are
defined in LOOP.md — never write those exact phrases here except to actually
fire them.

Human-action inbox (the loop adds items here it must not do itself —
deploys, Caddyfile prefixes, Peppol account, AI-provider keys):

- (none yet)

---

marathon preflight from the Mac, 2026-08-06 — toolchain and push access verified.
