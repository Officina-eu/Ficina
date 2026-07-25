//! The SMTP session state machine — pure protocol logic, no I/O.
//!
//! [`Session`] consumes complete command lines and produces replies;
//! the transport (its limits, timeouts, and sockets) lives in
//! [`crate::server`]. This split keeps every protocol decision
//! unit-testable without a socket.

use crate::command::{self, Command, CommandError};
use crate::reply::Reply;

/// What the transport must do after writing the reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Keep the connection open and read the next command.
    Continue,
    /// Close the connection (QUIT, RFC 5321 §4.1.1.10).
    Close,
}

/// One SMTP session's protocol state.
///
/// Phase 0 holds no transaction state yet — MAIL/RCPT/DATA arrive in
/// Phase 1 and with them the full state machine of RFC 5321 §4.1.4.
#[derive(Debug)]
pub struct Session {
    hostname: String,
}

impl Session {
    /// Creates a session for a server announcing `hostname`.
    pub fn new(hostname: impl Into<String>) -> Self {
        Self {
            hostname: hostname.into(),
        }
    }

    /// The 220 greeting sent when the connection opens
    /// (RFC 5321 §3.1).
    pub fn greeting(&self) -> Reply {
        Reply::service_ready(&self.hostname)
    }

    /// Handles one complete command line (CRLF already stripped) and
    /// returns the reply plus what the transport should do next.
    pub fn on_line(&mut self, line: &str) -> (Reply, Action) {
        match command::parse(line) {
            Ok(Command::Ehlo { client }) => {
                (Reply::ehlo_ok(&self.hostname, &client), Action::Continue)
            }
            Ok(Command::Quit) => (Reply::closing(&self.hostname), Action::Close),
            Ok(Command::Unknown { verb }) => {
                tracing::debug!(%verb, "unrecognized command");
                (Reply::command_unrecognized(), Action::Continue)
            }
            Err(CommandError::Empty) => (Reply::command_unrecognized(), Action::Continue),
            Err(CommandError::MissingParameter { .. } | CommandError::BadParameter { .. }) => {
                (Reply::parameter_error(), Action::Continue)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        Session::new("mx.ficina.test")
    }

    #[test]
    fn greeting_is_220() {
        assert_eq!(session().greeting().code(), 220);
    }

    #[test]
    fn ehlo_gets_250_and_continues() {
        let (reply, action) = session().on_line("EHLO client.example");
        assert_eq!(reply.code(), 250);
        assert_eq!(action, Action::Continue);
    }

    #[test]
    fn quit_gets_221_and_closes() {
        let (reply, action) = session().on_line("QUIT");
        assert_eq!(reply.code(), 221);
        assert_eq!(action, Action::Close);
    }

    #[test]
    fn unknown_command_gets_500_and_continues() {
        let (reply, action) = session().on_line("NOOP");
        assert_eq!(reply.code(), 500);
        assert_eq!(action, Action::Continue);
    }

    #[test]
    fn ehlo_without_domain_gets_501() {
        let (reply, action) = session().on_line("EHLO");
        assert_eq!(reply.code(), 501);
        assert_eq!(action, Action::Continue);
    }

    #[test]
    fn quit_with_argument_gets_501_and_stays_open() {
        let (reply, action) = session().on_line("QUIT later");
        assert_eq!(reply.code(), 501);
        assert_eq!(action, Action::Continue);
    }

    #[test]
    fn empty_line_gets_500() {
        let (reply, _) = session().on_line("");
        assert_eq!(reply.code(), 500);
    }
}
