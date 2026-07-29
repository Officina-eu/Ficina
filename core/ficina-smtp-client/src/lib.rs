//! Ficina outbound SMTP client — the protocol primitives shared by two
//! callers: the SMTP service's queue (MX delivery, `ficina-smtp`) and the JMAP
//! service's submission path (`ficina-jmap`, sending a composed message through
//! the trusted internal submission listener). One client, two callers, no
//! duplication.
//!
//! - [`line`] — bounded CRLF line reading.
//! - [`client_reply`] — SMTP multi-line reply parsing (`ServerReply`).
//! - [`client`] — [`client::OutboundSession`]: connect, EHLO/HELO, MAIL/RCPT/
//!   DATA with RFC 5321 §4.5.3.2 timeouts, dot-stuffing, and per-recipient
//!   4xx/5xx classification.

pub mod client;
pub mod client_reply;
pub mod line;
