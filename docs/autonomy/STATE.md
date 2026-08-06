# STATE.md — loop journal (append-only; newest at the bottom)

The loop appends one entry per iteration: item id, what shipped, how it was
verified, anything cut or flagged for human review, and the next item. Humans
read this file with morning coffee; the loop reads it to regain context.
Control markers the wrapper watches for: `LOOP COMPLETE`, `LOOP HALT: <reason>`.

Human-action inbox (the loop adds items here it must not do itself —
deploys, Caddyfile prefixes, Peppol account, AI-provider keys):

- (none yet)

---
