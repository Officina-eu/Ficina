# ADR 0007 — Sieve rule management over JMAP, not ManageSieve

**Status:** accepted · 2026-07

**Decision:** User Sieve scripts are managed through **JMAP for Sieve
(RFC 9661)** — `SieveScript/{get,set,query,validate}` and an active-script
setting on the mail account — not through **ManageSieve (RFC 5804)**.
Scripts are per-account data on `AccountStore`; a script is compile-checked
on upload and only a script that compiles can be activated.

**Why:** ADR 0001 makes JMAP the native client protocol: one authenticated
connection over 443, delta sync, and one id space, with IMAP/POP3 as shims.
Sieve management is the same story. RFC 9661 was designed to sit exactly
where our `SieveScript` objects already want to live — beside `Mailbox`
and `Email`, reached through the same bearer-auth `AccountStore` door, so
it inherits isolation and change-tracking for free and our own web UI edits
it later without a second protocol. It reuses the compile-on-`set`
validation the store already needs (`invalidScript` maps to a JMAP
`SetError`).

**Rejected — ManageSieve (RFC 5804):** a fourth TLS listener speaking its
own line protocol with its own SASL stack, whose sole near-term advantage
is that third-party editors (the Thunderbird *Sieve* add-on, `sieve-connect`)
speak it. We do not need those editors to ship filtering — our clients are
web-first (ADR 0005) and drive JMAP. ManageSieve would be a second auth
model to build and migrate, duplicating the credential seam ficina-identity
will own, for an audience we can serve additively later. It is not
foreclosed: if a customer needs a desktop Sieve editor, ManageSieve is a
self-contained new listener that translates the same `AccountStore` script
operations — added when the demand is real, not speculatively.

**Rejected — a Ficina-proprietary rules API instead of RFC 9661:** it would
be JMAP-shaped but non-standard, forfeiting the one reason to pick JMAP
(a real, interoperable spec). RFC 9661 costs nothing extra over a
home-rolled shape and keeps us honest.

**Consequences:** the `SieveScript` object and its `set` validation are a
public contract from merge (they change additively, per CLAUDE.md). The
Sieve *engine* (`ficina-sieve`) is protocol-agnostic — it compiles and
evaluates scripts and knows nothing about JMAP or ManageSieve — so the
management protocol is a thin adapter over it, and adding ManageSieve later
touches no engine code. See `docs/design/sieve-filtering.md`.
