//! Parsing of SMTP command lines (RFC 5321 §4.1.1).
//!
//! Parsing is separated from the session state machine so each can be
//! tested alone: this module decides *what the client said*, never
//! what to do about it.

/// A syntactically valid command in the current Phase 0 vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `EHLO <domain>` (RFC 5321 §4.1.1.1).
    Ehlo {
        /// The client-supplied domain or address literal, verbatim.
        client: String,
    },
    /// `QUIT` (RFC 5321 §4.1.1.10).
    Quit,
    /// A verb we do not implement (or do not implement yet).
    Unknown {
        /// The verb as received, for logging — never echoed to the peer.
        verb: String,
    },
}

/// A command line that could not be accepted as written.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommandError {
    /// Empty command line.
    #[error("empty command line")]
    Empty,
    /// A verb that requires an argument arrived without one
    /// (EHLO requires a domain, RFC 5321 §4.1.1.1).
    #[error("{verb} requires an argument")]
    MissingParameter {
        /// The verb in question.
        verb: String,
    },
    /// A verb received arguments it does not take, or malformed ones
    /// (QUIT takes none, RFC 5321 §4.1.1.10).
    #[error("{verb} received unexpected or malformed arguments")]
    BadParameter {
        /// The verb in question.
        verb: String,
    },
}

/// Parses one command line (already stripped of its CRLF).
///
/// Verbs are case-insensitive (RFC 5321 §2.4). The verb and any
/// argument are separated by a single space (§4.1.1).
///
/// # Errors
/// Returns [`CommandError`] when the line is empty or the arguments
/// do not fit the verb; the session maps these to 500/501 replies.
pub fn parse(line: &str) -> Result<Command, CommandError> {
    if line.is_empty() {
        return Err(CommandError::Empty);
    }

    let (verb, argument) = match line.split_once(' ') {
        Some((v, rest)) => (v, Some(rest)),
        None => (line, None),
    };
    let verb_upper = verb.to_ascii_uppercase();

    match verb_upper.as_str() {
        "EHLO" => match argument {
            None | Some("") => Err(CommandError::MissingParameter { verb: verb_upper }),
            // Exactly one Domain / address-literal token (§4.1.1.1).
            Some(client) if client.contains(' ') => {
                Err(CommandError::BadParameter { verb: verb_upper })
            }
            Some(client) => Ok(Command::Ehlo {
                client: client.to_owned(),
            }),
        },
        "QUIT" => match argument {
            // "QUIT CRLF" admits no arguments (§4.1.1.10).
            None => Ok(Command::Quit),
            Some(_) => Err(CommandError::BadParameter { verb: verb_upper }),
        },
        _ => Ok(Command::Unknown { verb: verb_upper }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ehlo_with_domain_parses() {
        assert_eq!(
            parse("EHLO client.example"),
            Ok(Command::Ehlo {
                client: "client.example".to_owned()
            })
        );
    }

    #[test]
    fn verbs_are_case_insensitive() {
        // RFC 5321 §2.4
        assert_eq!(
            parse("ehlo client.example"),
            Ok(Command::Ehlo {
                client: "client.example".to_owned()
            })
        );
        assert_eq!(parse("qUiT"), Ok(Command::Quit));
    }

    #[test]
    fn ehlo_without_domain_is_missing_parameter() {
        assert_eq!(
            parse("EHLO"),
            Err(CommandError::MissingParameter {
                verb: "EHLO".to_owned()
            })
        );
        assert_eq!(
            parse("EHLO "),
            Err(CommandError::MissingParameter {
                verb: "EHLO".to_owned()
            })
        );
    }

    #[test]
    fn ehlo_with_two_tokens_is_bad_parameter() {
        assert_eq!(
            parse("EHLO a b"),
            Err(CommandError::BadParameter {
                verb: "EHLO".to_owned()
            })
        );
    }

    #[test]
    fn quit_with_argument_is_bad_parameter() {
        // RFC 5321 §4.1.1.10: QUIT takes no arguments.
        assert_eq!(
            parse("QUIT now"),
            Err(CommandError::BadParameter {
                verb: "QUIT".to_owned()
            })
        );
    }

    #[test]
    fn unknown_verb_is_reported_uppercased() {
        assert_eq!(
            parse("mail FROM:<a@b>"),
            Ok(Command::Unknown {
                verb: "MAIL".to_owned()
            })
        );
    }

    #[test]
    fn empty_line_is_rejected() {
        assert_eq!(parse(""), Err(CommandError::Empty));
    }
}
