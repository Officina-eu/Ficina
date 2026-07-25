# ADR 0005 — Web-first clients; Tauri shell; two languages forever

**Status:** accepted · 2026-07

**Decision:** One TypeScript web application is the product UI
everywhere: browser, installable PWA at launch, Tauri desktop shell in
phase two (tray, native notifications, autostart), mobile shells
after. Offline-first desktop arrives later as a local Rust cache
syncing over JMAP, sharing types with ficina-core. Repo languages are
Rust and TypeScript, exclusively.

**Why:** Data sovereignty is a server-side promise (Self-Hosted
edition) — the client changes nothing about where data lives, so the
client question is pure UX and team economics: one codebase ships
every feature to every platform simultaneously, which is the only
model a two-person team survives. Tauri over Electron: Rust (our
stack), ~10MB not ~150MB. Outlook users already get a native app via
the compat adapters.

**Rejected:** desktop-first (conflates client with sovereignty; triples
maintenance); Electron (weight, second runtime); native per-platform
apps and Flutter (third language, forbidden).

**Consequences:** notifications/presence must be first-class in the
PWA until the Tauri shell lands; the offline cache is a roadmap item
with its own design review, not an assumption.
