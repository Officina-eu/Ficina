//! # ficina-smtp — mail transfer and submission
//!
//! Owns: SMTP receiving (port 25), client submission (587), queueing,
//! routing, and retries (ARCHITECTURE.md). Does not own: message
//! storage (`ficina-store`), DKIM/SPF/DMARC (`ficina-auth-mail`), or
//! client APIs (`ficina-jmap`).
//!
//! Phase 0 scope (ROADMAP.md exit gate): a session skeleton that
//! greets, negotiates EHLO, and quits with RFC 5321-correct replies.
//! The full session state machine, queueing, and the trust stack are
//! Phase 1 items.
//!
//! Layering: [`session`] is the pure protocol state machine (no I/O),
//! [`server`] puts it on a TCP socket with the read limits and
//! timeouts RFC 5321 requires, [`reply`] and [`command`] are the wire
//! vocabulary.

pub mod command;
pub mod config;
pub mod error;
pub mod healthcheck;
pub mod reply;
pub mod server;
pub mod session;
