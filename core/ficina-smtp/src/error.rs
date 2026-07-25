//! The crate's error type.

use std::net::SocketAddr;

/// Errors surfaced by the ficina-smtp service.
///
/// Per-connection I/O failures are not represented here: a peer
/// disconnecting is a normal event handled inside the connection task,
/// never a service error.
#[derive(Debug, thiserror::Error)]
pub enum SmtpError {
    /// Configuration was present but unusable; the message names the
    /// variable and the expected form.
    #[error("configuration error: {message}")]
    Config {
        /// Actionable description for the operator.
        message: String,
    },

    /// The listener could not bind its address.
    #[error("failed to bind {addr} (is the port in use, or the address unavailable?): {source}")]
    Bind {
        /// Address the bind was attempted on.
        addr: SocketAddr,
        /// Underlying OS error.
        #[source]
        source: std::io::Error,
    },

    /// The health probe could not confirm a live SMTP greeting.
    #[error("health probe against {addr} failed: {reason}")]
    Unhealthy {
        /// Address that was probed.
        addr: SocketAddr,
        /// What the probe observed instead of a 220 greeting.
        reason: String,
    },
}
