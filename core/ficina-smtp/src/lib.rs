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

pub mod address;
pub mod auth;
pub mod authmail;
pub mod backoff;
pub mod client;
pub mod client_reply;
pub mod command;
pub mod config;
pub mod data;
pub mod dsn;
pub mod envelope;
pub mod error;
pub mod healthcheck;
mod line;
pub mod queue;
pub mod queue_runner;
pub mod received;
pub mod reply;
pub mod resolver;
pub mod server;
pub mod session;
pub mod spool;
pub mod stream;
pub mod submission;
pub mod tls;
