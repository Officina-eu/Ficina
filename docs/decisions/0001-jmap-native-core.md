# ADR 0001 — JMAP-native mail core, from scratch, in Rust

**Status:** accepted · 2026-07

**Decision:** Build the mail core ourselves (SMTP, store, JMAP) in
Rust, with JMAP as the native API and IMAP/POP3 as compatibility shims
over the store.

**Why:** The mail core and AI layer are the two places Ficina
differentiates; everything else is commodity. JMAP (RFC 8620/8621)
fixes IMAP's real defects — single connection over 443, true push,
delta sync, sane blobs, one protocol family extending to
calendar/contacts — and is designed for exactly the offline-capable
clients we plan. Building on an existing engine (Stalwart, grommunio)
would tie our differentiation roadmap to someone else's.

**Rejected:** grommunio as base (older stack; our value would sit on
rented foundations); Stalwart as base (excellent, but the core IS our
product — and it remains the fallback if we ever must retreat).

**Consequences:** ~6 months of core work before the product layer;
IMAP clients are second-class by architecture (shim), which we accept;
Exchange adapters (EAS/MAPI) become edge translators to JMAP, year two.
