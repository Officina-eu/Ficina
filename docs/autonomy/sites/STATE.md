# sites/STATE.md — Sites-track loop journal (append-only; newest at the bottom)

One entry per iteration: item id, what shipped, how verified, cuts/flags,
next item. Control markers: `LOOP COMPLETE`, `LOOP HALT: <reason>`.

Human-action inbox (things the loop must not do itself):

- Buy/choose the public sites domain (e.g. alosites.com) and set wildcard DNS
  — needed before any real subdomain goes live (ADR 0036 open decision).
- At next deploy: add the alo-sites container to production compose + Caddy
  wildcard/on-demand-TLS config (the loop never touches deploy/).
- Configure an AI provider key on the live server before real "generate my
  site" runs (loop verifies with fixtures only).

---
