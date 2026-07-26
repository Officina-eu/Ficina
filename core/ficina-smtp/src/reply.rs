//! SMTP reply construction and wire formatting (RFC 5321 §4.2).
//!
//! Every reply the server can currently send is built here, so reply
//! codes have exactly one source and each carries its RFC citation.

use std::fmt;

/// A single-line SMTP reply: three-digit code plus human-readable text.
///
/// Multi-line replies (§4.2.1 `code-`) arrive with EHLO capability
/// advertising in Phase 1; nothing in the current scope needs them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    code: u16,
    text: String,
}

impl Reply {
    /// 220: service ready — the opening banner (RFC 5321 §3.1, §4.2.3).
    pub fn service_ready(hostname: &str) -> Self {
        Self {
            code: 220,
            text: format!("{hostname} ESMTP Ficina"),
        }
    }

    /// 250: EHLO accepted (RFC 5321 §4.1.1.1). No extensions are
    /// advertised yet, so the reply is a single line (advertisement
    /// becomes truthful and complete in the TLS/submission milestone).
    pub fn ehlo_ok(hostname: &str, client: &str) -> Self {
        Self {
            code: 250,
            text: format!("{hostname} greets {client}"),
        }
    }

    /// 250: HELO accepted (RFC 5321 §4.1.1.1 — the reply carries only
    /// our domain for pre-ESMTP clients).
    pub fn helo_ok(hostname: &str) -> Self {
        Self {
            code: 250,
            text: hostname.to_owned(),
        }
    }

    /// 250: generic success for MAIL/RCPT/RSET/NOOP (§4.1.1).
    pub fn ok() -> Self {
        Self {
            code: 250,
            text: "OK".to_owned(),
        }
    }

    /// 250: message accepted and durably spooled, with its id.
    pub fn ok_queued(id: &str) -> Self {
        Self {
            code: 250,
            text: format!("OK: queued as {id}"),
        }
    }

    /// 354: start mail input (RFC 5321 §4.1.1.4).
    pub fn start_mail_input() -> Self {
        Self {
            code: 354,
            text: "Start mail input; end with <CRLF>.<CRLF>".to_owned(),
        }
    }

    /// 503: bad sequence of commands (RFC 5321 §4.1.4).
    pub fn bad_sequence(hint: &str) -> Self {
        Self {
            code: 503,
            text: format!("bad sequence of commands: {hint}"),
        }
    }

    /// 555: MAIL/RCPT parameters not recognized or not implemented
    /// (RFC 5321 §4.1.1.11) — sent because no extension is advertised.
    pub fn params_not_recognized() -> Self {
        Self {
            code: 555,
            text: "MAIL FROM/RCPT TO parameters not recognized or not implemented".to_owned(),
        }
    }

    /// 452: too many recipients (RFC 5321 §4.5.3.1.10 — transient by
    /// design so the client retries the rest in a new transaction).
    pub fn too_many_recipients() -> Self {
        Self {
            code: 452,
            text: "too many recipients".to_owned(),
        }
    }

    /// 552: message exceeds the fixed maximum message size
    /// (RFC 1870 semantics; the limit is enforced during read).
    pub fn message_too_large() -> Self {
        Self {
            code: 552,
            text: "message exceeds fixed maximum message size".to_owned(),
        }
    }

    /// 502: command recognized but not implemented (RFC 5321 §4.2.4).
    pub fn not_implemented() -> Self {
        Self {
            code: 502,
            text: "command not implemented".to_owned(),
        }
    }

    /// 252: VRFY answered without disclosing user existence
    /// (RFC 5321 §3.5.3, §7.3 — anti-enumeration).
    pub fn vrfy_noncommittal() -> Self {
        Self {
            code: 252,
            text: "cannot VRFY user, but will accept message and attempt delivery".to_owned(),
        }
    }

    /// 451: local error in processing (RFC 5321 §4.2.4) — transient,
    /// used when the spool write fails so the client retries.
    pub fn local_error() -> Self {
        Self {
            code: 451,
            text: "local error in processing, try again later".to_owned(),
        }
    }

    /// 221: closing transmission channel, response to QUIT
    /// (RFC 5321 §4.1.1.10).
    pub fn closing(hostname: &str) -> Self {
        Self {
            code: 221,
            text: format!("{hostname} closing transmission channel"),
        }
    }

    /// 500: syntax error, command unrecognized (RFC 5321 §4.2.4).
    pub fn command_unrecognized() -> Self {
        Self {
            code: 500,
            text: "syntax error, command unrecognized".to_owned(),
        }
    }

    /// 500: command line exceeded the 512-octet limit of
    /// RFC 5321 §4.5.3.1.4.
    pub fn line_too_long() -> Self {
        Self {
            code: 500,
            text: "line too long".to_owned(),
        }
    }

    /// 500: line ending was not CRLF. RFC 5321 §2.3.8 requires CRLF;
    /// accepting bare LF/CR enables SMTP smuggling, so we reject
    /// rather than guess (protocol skill: reject when ambiguity has
    /// security consequences).
    pub fn bare_line_ending() -> Self {
        Self {
            code: 500,
            text: "line ending must be CRLF".to_owned(),
        }
    }

    /// 501: syntax error in parameters or arguments (RFC 5321 §4.2.4).
    pub fn parameter_error() -> Self {
        Self {
            code: 501,
            text: "syntax error in parameters or arguments".to_owned(),
        }
    }

    /// 421: service closing — sent when the server must drop the
    /// connection (idle timeout per RFC 5321 §4.5.3.2, or flooding).
    pub fn service_closing(hostname: &str) -> Self {
        Self {
            code: 421,
            text: format!("{hostname} service closing transmission channel"),
        }
    }

    /// The reply's three-digit code.
    pub fn code(&self) -> u16 {
        self.code
    }
}

impl fmt::Display for Reply {
    /// Wire form: `code SP text CRLF` (RFC 5321 §4.2). CRLF is the
    /// line ending, always, both directions (protocol skill).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}\r\n", self.code, self.text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_form_ends_with_crlf() {
        let wire = Reply::service_ready("mx.example").to_string();
        assert_eq!(wire, "220 mx.example ESMTP Ficina\r\n");
        assert!(wire.ends_with("\r\n"));
    }

    #[test]
    fn codes_match_rfc_5321() {
        assert_eq!(Reply::service_ready("h").code(), 220);
        assert_eq!(Reply::ehlo_ok("h", "c").code(), 250);
        assert_eq!(Reply::closing("h").code(), 221);
        assert_eq!(Reply::command_unrecognized().code(), 500);
        assert_eq!(Reply::line_too_long().code(), 500);
        assert_eq!(Reply::bare_line_ending().code(), 500);
        assert_eq!(Reply::parameter_error().code(), 501);
        assert_eq!(Reply::service_closing("h").code(), 421);
    }
}
