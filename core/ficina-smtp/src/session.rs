//! The SMTP session state machine — pure protocol logic, no I/O
//! (RFC 5321 §4.1.4 command ordering).
//!
//! [`Session`] consumes complete command lines and produces replies;
//! the transport (limits, timeouts, sockets, the DATA byte stream)
//! lives in [`crate::server`].

use crate::address::{ForwardPath, ReversePath};
use crate::command::{self, Command, CommandError};
use crate::reply::Reply;

/// What the transport must do after writing the reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Keep reading commands.
    Continue,
    /// Close the connection (QUIT).
    Close,
    /// Switch to DATA collection (the 354 reply is already returned).
    EnterData,
}

/// Transaction state (§4.1.4): MAIL begins one, RCPT extends it, DATA
/// consumes it, RSET/EHLO/HELO abort it.
#[derive(Debug)]
enum Txn {
    Idle,
    InProgress {
        from: ReversePath,
        rcpts: Vec<ForwardPath>,
    },
}

/// One SMTP session's protocol state.
#[derive(Debug)]
pub struct Session {
    hostname: String,
    max_rcpt: usize,
    /// EHLO/HELO argument once greeted; commands requiring a greeting
    /// are rejected 503 until set.
    helo: Option<String>,
    /// Whether the greeting was EHLO (ESMTP) or HELO (SMTP) — drives
    /// the WITH clause of the Received: stamp (RFC 3848).
    esmtp: bool,
    txn: Txn,
    /// SMTPUTF8 is negotiated per transaction (RFC 6531); never
    /// enabled until the extension is advertised (M3).
    utf8_enabled: bool,
}

impl Session {
    /// Creates a session for a server announcing `hostname`, accepting
    /// at most `max_rcpt` recipients per transaction (§4.5.3.1.8:
    /// minimum 100).
    pub fn new(hostname: impl Into<String>, max_rcpt: usize) -> Self {
        Self {
            hostname: hostname.into(),
            max_rcpt,
            helo: None,
            esmtp: false,
            txn: Txn::Idle,
            utf8_enabled: false,
        }
    }

    /// The 220 greeting sent when the connection opens (§3.1).
    pub fn greeting(&self) -> Reply {
        Reply::service_ready(&self.hostname)
    }

    /// Handles one complete command line (CRLF already stripped).
    pub fn on_line(&mut self, line: &str) -> (Reply, Action) {
        let command = match command::parse(line, self.utf8_enabled) {
            Ok(command) => command,
            Err(CommandError::Empty) => return (Reply::command_unrecognized(), Action::Continue),
            Err(CommandError::BadAddress(error)) => {
                tracing::debug!(%error, "address rejected");
                return (Reply::parameter_error(), Action::Continue);
            }
            Err(CommandError::MissingParameter { .. } | CommandError::BadParameter { .. }) => {
                return (Reply::parameter_error(), Action::Continue);
            }
        };

        match command {
            Command::Ehlo { client } => {
                // EHLO aborts any transaction in progress (§4.1.4).
                self.txn = Txn::Idle;
                self.helo = Some(client.clone());
                self.esmtp = true;
                (Reply::ehlo_ok(&self.hostname, &client), Action::Continue)
            }
            Command::Helo { client } => {
                self.txn = Txn::Idle;
                self.helo = Some(client);
                self.esmtp = false;
                (Reply::helo_ok(&self.hostname), Action::Continue)
            }
            Command::Mail {
                reverse_path,
                params,
            } => {
                if self.helo.is_none() {
                    return (
                        Reply::bad_sequence("send EHLO or HELO first"),
                        Action::Continue,
                    );
                }
                if !matches!(self.txn, Txn::Idle) {
                    return (
                        Reply::bad_sequence("MAIL transaction already in progress"),
                        Action::Continue,
                    );
                }
                // No extensions are advertised, so no parameters are
                // acceptable (§4.1.1.11 → 555).
                if params.is_some() {
                    return (Reply::params_not_recognized(), Action::Continue);
                }
                self.txn = Txn::InProgress {
                    from: reverse_path,
                    rcpts: Vec::new(),
                };
                (Reply::ok(), Action::Continue)
            }
            Command::Rcpt {
                forward_path,
                params,
            } => {
                let Txn::InProgress { rcpts, .. } = &mut self.txn else {
                    return (
                        Reply::bad_sequence("need MAIL before RCPT"),
                        Action::Continue,
                    );
                };
                if params.is_some() {
                    return (Reply::params_not_recognized(), Action::Continue);
                }
                if rcpts.len() >= self.max_rcpt {
                    // §4.5.3.1.10: 452 means "try the rest in a new
                    // transaction", not a permanent failure.
                    return (Reply::too_many_recipients(), Action::Continue);
                }
                rcpts.push(forward_path);
                (Reply::ok(), Action::Continue)
            }
            Command::Data => match &self.txn {
                Txn::InProgress { rcpts, .. } if !rcpts.is_empty() => {
                    (Reply::start_mail_input(), Action::EnterData)
                }
                Txn::InProgress { .. } => (
                    Reply::bad_sequence("need at least one RCPT before DATA"),
                    Action::Continue,
                ),
                Txn::Idle => (
                    Reply::bad_sequence("need MAIL before DATA"),
                    Action::Continue,
                ),
            },
            Command::Rset => {
                self.txn = Txn::Idle;
                (Reply::ok(), Action::Continue)
            }
            Command::Noop => (Reply::ok(), Action::Continue),
            // §4.1.1.6: 252 discloses nothing about user existence.
            Command::Vrfy => (Reply::vrfy_noncommittal(), Action::Continue),
            Command::Quit => (Reply::closing(&self.hostname), Action::Close),
            Command::NotImplemented { verb } => {
                tracing::debug!(%verb, "recognized but unimplemented command");
                (Reply::not_implemented(), Action::Continue)
            }
            Command::Unknown { verb } => {
                tracing::debug!(%verb, "unrecognized command");
                (Reply::command_unrecognized(), Action::Continue)
            }
        }
    }

    /// The HELO/EHLO identity, for the `Received:` stamp.
    pub fn helo_client(&self) -> &str {
        self.helo.as_deref().unwrap_or("unknown")
    }

    /// WITH-clause protocol name for the `Received:` stamp
    /// (RFC 3848: `ESMTP` after EHLO, `SMTP` after HELO).
    pub fn protocol_name(&self) -> &'static str {
        if self.esmtp { "ESMTP" } else { "SMTP" }
    }

    /// Envelope fields of the in-flight transaction: sender display
    /// form (`None` = null path) and recipient display forms. `None`
    /// when no transaction is in progress.
    pub fn envelope_fields(&self) -> Option<(Option<String>, Vec<String>)> {
        match &self.txn {
            Txn::InProgress { from, rcpts } => {
                let from = match from {
                    ReversePath::Null => None,
                    ReversePath::Mailbox(m) => Some(m.to_string()),
                };
                let rcpts = rcpts
                    .iter()
                    .map(|r| match r {
                        ForwardPath::Postmaster => "postmaster".to_owned(),
                        ForwardPath::Mailbox(m) => m.to_string(),
                    })
                    .collect();
                Some((from, rcpts))
            }
            Txn::Idle => None,
        }
    }

    /// Ends the DATA phase (success or failure): the transaction is
    /// consumed either way (§4.1.1.4).
    pub fn end_data(&mut self) {
        self.txn = Txn::Idle;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn greeted() -> Session {
        let mut s = Session::new("mx.ficina.test", 100);
        let (reply, _) = s.on_line("EHLO client.example");
        assert_eq!(reply.code(), 250);
        s
    }

    fn with_rcpt() -> Session {
        let mut s = greeted();
        assert_eq!(s.on_line("MAIL FROM:<bob@example.org>").0.code(), 250);
        assert_eq!(s.on_line("RCPT TO:<alice@example.com>").0.code(), 250);
        s
    }

    #[test]
    fn full_transaction_reaches_data() {
        let mut s = with_rcpt();
        let (reply, action) = s.on_line("DATA");
        assert_eq!(reply.code(), 354);
        assert_eq!(action, Action::EnterData);
        let (from, rcpts) = s.envelope_fields().unwrap();
        assert_eq!(from.as_deref(), Some("bob@example.org"));
        assert_eq!(rcpts, vec!["alice@example.com"]);
    }

    #[test]
    fn mail_before_greeting_is_503() {
        let mut s = Session::new("mx.ficina.test", 100);
        assert_eq!(s.on_line("MAIL FROM:<bob@example.org>").0.code(), 503);
    }

    #[test]
    fn rcpt_before_mail_is_503() {
        let mut s = greeted();
        assert_eq!(s.on_line("RCPT TO:<alice@example.com>").0.code(), 503);
    }

    #[test]
    fn data_before_rcpt_is_503() {
        let mut s = greeted();
        assert_eq!(s.on_line("DATA").0.code(), 503);
        assert_eq!(s.on_line("MAIL FROM:<bob@example.org>").0.code(), 250);
        assert_eq!(s.on_line("DATA").0.code(), 503);
    }

    #[test]
    fn nested_mail_is_503() {
        let mut s = with_rcpt();
        assert_eq!(s.on_line("MAIL FROM:<carol@example.org>").0.code(), 503);
    }

    #[test]
    fn rset_aborts_the_transaction() {
        let mut s = with_rcpt();
        assert_eq!(s.on_line("RSET").0.code(), 250);
        assert_eq!(s.on_line("DATA").0.code(), 503);
        assert!(s.envelope_fields().is_none());
    }

    #[test]
    fn ehlo_mid_transaction_aborts_it() {
        // §4.1.4: EHLO resets state.
        let mut s = with_rcpt();
        assert_eq!(s.on_line("EHLO other.example").0.code(), 250);
        assert_eq!(s.on_line("DATA").0.code(), 503);
    }

    #[test]
    fn null_sender_is_accepted() {
        let mut s = greeted();
        assert_eq!(s.on_line("MAIL FROM:<>").0.code(), 250);
        assert_eq!(s.on_line("RCPT TO:<alice@example.com>").0.code(), 250);
        let (from, _) = s.envelope_fields().unwrap();
        assert!(from.is_none());
    }

    #[test]
    fn esmtp_params_get_555_when_none_advertised() {
        let mut s = greeted();
        assert_eq!(
            s.on_line("MAIL FROM:<bob@example.org> SIZE=1000").0.code(),
            555
        );
        // The rejected MAIL must not have started a transaction.
        assert_eq!(s.on_line("RCPT TO:<alice@example.com>").0.code(), 503);
    }

    #[test]
    fn recipient_limit_is_452() {
        let mut s = Session::new("mx.ficina.test", 2);
        s.on_line("EHLO client.example");
        s.on_line("MAIL FROM:<bob@example.org>");
        assert_eq!(s.on_line("RCPT TO:<a@example.com>").0.code(), 250);
        assert_eq!(s.on_line("RCPT TO:<b@example.com>").0.code(), 250);
        assert_eq!(s.on_line("RCPT TO:<c@example.com>").0.code(), 452);
    }

    #[test]
    fn postmaster_rcpt_is_accepted() {
        let mut s = greeted();
        s.on_line("MAIL FROM:<bob@example.org>");
        assert_eq!(s.on_line("RCPT TO:<postmaster>").0.code(), 250);
        let (_, rcpts) = s.envelope_fields().unwrap();
        assert_eq!(rcpts, vec!["postmaster"]);
    }

    #[test]
    fn bad_address_is_501_and_state_unchanged() {
        let mut s = greeted();
        assert_eq!(s.on_line("MAIL FROM:<broken>").0.code(), 501);
        assert_eq!(s.on_line("MAIL FROM:<bob@example.org>").0.code(), 250);
    }

    #[test]
    fn service_commands_reply_correctly() {
        let mut s = greeted();
        assert_eq!(s.on_line("NOOP").0.code(), 250);
        assert_eq!(s.on_line("VRFY alice").0.code(), 252);
        assert_eq!(s.on_line("HELP").0.code(), 502);
        assert_eq!(s.on_line("XYZZY").0.code(), 500);
        let (reply, action) = s.on_line("QUIT");
        assert_eq!(reply.code(), 221);
        assert_eq!(action, Action::Close);
    }

    #[test]
    fn end_data_consumes_the_transaction() {
        let mut s = with_rcpt();
        assert_eq!(s.on_line("DATA").1, Action::EnterData);
        s.end_data();
        assert!(s.envelope_fields().is_none());
        assert_eq!(s.on_line("DATA").0.code(), 503);
    }
}
