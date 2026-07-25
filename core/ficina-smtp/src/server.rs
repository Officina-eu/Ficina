//! TCP transport: the listener, per-connection wire I/O, and the
//! read-side limits and timeouts RFC 5321 requires.
//!
//! All protocol decisions live in [`crate::session`]; this module only
//! moves bytes safely.

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tracing::Instrument;

use crate::config::SmtpConfig;
use crate::error::SmtpError;
use crate::reply::Reply;
use crate::session::{Action, Session};

/// Command line limit: 512 octets including CRLF
/// (RFC 5321 §4.5.3.1.4). Enforced during read, never after buffering
/// (protocol skill non-negotiable).
const MAX_COMMAND_LINE: usize = 512;

/// Hard ceiling on octets drained while looking for the end of an
/// over-long line; a peer exceeding it is flooding and is dropped.
const FLOOD_LIMIT: usize = 64 * 1024;

/// Server-side idle timeout, RFC 5321 §4.5.3.2 (5 minutes minimum for
/// most waits; we apply it uniformly while awaiting a command).
const COMMAND_TIMEOUT: Duration = Duration::from_secs(300);

/// Delay before re-accepting after a transient `accept()` error, so a
/// resource-exhaustion condition cannot spin the accept loop hot.
const ACCEPT_RETRY_DELAY: Duration = Duration::from_millis(100);

/// Outcome of reading one command line under the RFC limits.
#[derive(Debug, PartialEq, Eq)]
enum LineOutcome {
    /// A complete, CRLF-terminated, ASCII command line (CRLF stripped).
    Line(String),
    /// Line exceeded [`MAX_COMMAND_LINE`]; the excess was drained.
    TooLong,
    /// Line ended in bare LF, or contained a stray CR — rejected as an
    /// SMTP-smuggling defense (RFC 5321 §2.3.8; protocol skill:
    /// reject, do not guess, when ambiguity has security consequences).
    BadLineEnding,
    /// Line contained non-ASCII octets; commands are ASCII
    /// (RFC 5321 §2.4). SMTPUTF8 (RFC 6531) is a Phase 1 extension.
    NotAscii,
    /// Peer exceeded [`FLOOD_LIMIT`] without ever sending a newline.
    Flooded,
    /// Peer closed the connection.
    Eof,
}

/// Binds the configured address and serves connections forever.
///
/// # Errors
/// Returns [`SmtpError::Bind`] when the listener cannot bind; once
/// bound, per-connection failures are logged and never fatal.
pub async fn run(config: SmtpConfig) -> Result<(), SmtpError> {
    let listener = TcpListener::bind(config.bind_addr)
        .await
        .map_err(|source| SmtpError::Bind {
            addr: config.bind_addr,
            source,
        })?;
    tracing::info!(addr = %config.bind_addr, hostname = %config.hostname, "ficina-smtp listening");
    serve(listener, Arc::new(config)).await
}

/// Accept loop over an already-bound listener (also the seam the
/// integration tests use to serve on an ephemeral port).
///
/// # Errors
/// Never returns an error today; the `Result` is the stable signature
/// for when shutdown handling lands (Phase 1).
pub async fn serve(listener: TcpListener, config: Arc<SmtpConfig>) -> Result<(), SmtpError> {
    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                let config = Arc::clone(&config);
                let span = tracing::info_span!("smtp_session", %peer);
                tokio::spawn(
                    async move {
                        if let Err(error) = handle_connection(stream, config).await {
                            // Peer-side I/O failures are normal churn,
                            // not service errors.
                            tracing::debug!(%error, "session ended with I/O error");
                        }
                    }
                    .instrument(span),
                );
            }
            Err(error) => {
                tracing::warn!(%error, "accept failed; retrying");
                tokio::time::sleep(ACCEPT_RETRY_DELAY).await;
            }
        }
    }
}

/// Drives one connection: greeting, then command/reply until close.
async fn handle_connection(stream: TcpStream, config: Arc<SmtpConfig>) -> std::io::Result<()> {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut session = Session::new(config.hostname.clone());

    tracing::info!("session opened");
    write_reply(&mut write_half, &session.greeting()).await?;

    loop {
        let outcome = match tokio::time::timeout(COMMAND_TIMEOUT, read_line(&mut reader)).await {
            Ok(result) => result?,
            Err(_elapsed) => {
                // RFC 5321 §4.5.3.2: on timeout, close with 421.
                tracing::info!("idle timeout; closing");
                write_reply(&mut write_half, &Reply::service_closing(&config.hostname)).await?;
                break;
            }
        };

        match outcome {
            LineOutcome::Line(line) => {
                let (reply, action) = session.on_line(&line);
                write_reply(&mut write_half, &reply).await?;
                if action == Action::Close {
                    tracing::info!("session closed by QUIT");
                    break;
                }
            }
            LineOutcome::TooLong => {
                write_reply(&mut write_half, &Reply::line_too_long()).await?;
            }
            LineOutcome::BadLineEnding => {
                write_reply(&mut write_half, &Reply::bare_line_ending()).await?;
            }
            LineOutcome::NotAscii => {
                write_reply(&mut write_half, &Reply::command_unrecognized()).await?;
            }
            LineOutcome::Flooded => {
                tracing::warn!("peer flooded without newline; closing");
                write_reply(&mut write_half, &Reply::service_closing(&config.hostname)).await?;
                break;
            }
            LineOutcome::Eof => {
                tracing::info!("peer disconnected");
                break;
            }
        }
    }
    Ok(())
}

async fn write_reply(writer: &mut OwnedWriteHalf, reply: &Reply) -> std::io::Result<()> {
    writer.write_all(reply.to_string().as_bytes()).await?;
    writer.flush().await
}

/// Reads one command line, enforcing [`MAX_COMMAND_LINE`] during the
/// read and draining over-long input up to [`FLOOD_LIMIT`].
async fn read_line<R>(reader: &mut R) -> std::io::Result<LineOutcome>
where
    R: AsyncBufRead + Unpin,
{
    let mut line: Vec<u8> = Vec::with_capacity(128);
    let mut overflowed = false;
    let mut drained: usize = 0;

    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            // EOF mid-line is still EOF: an unterminated command is
            // never dispatched.
            return Ok(LineOutcome::Eof);
        }

        if let Some(newline_at) = available.iter().position(|&b| b == b'\n') {
            if !overflowed {
                line.extend_from_slice(&available[..=newline_at]);
            }
            reader.consume(newline_at + 1);

            if overflowed || line.len() > MAX_COMMAND_LINE {
                return Ok(LineOutcome::TooLong);
            }
            // Strip and verify the terminator: must be exactly CRLF.
            if line.len() < 2 || line[line.len() - 2] != b'\r' {
                return Ok(LineOutcome::BadLineEnding);
            }
            line.truncate(line.len() - 2);
            if line.contains(&b'\r') {
                return Ok(LineOutcome::BadLineEnding);
            }
            if !line.is_ascii() {
                return Ok(LineOutcome::NotAscii);
            }
            // ASCII just verified, so this conversion is lossless.
            return Ok(LineOutcome::Line(
                String::from_utf8_lossy(&line).into_owned(),
            ));
        }

        let chunk = available.len();
        if !overflowed {
            line.extend_from_slice(available);
            if line.len() > MAX_COMMAND_LINE {
                overflowed = true;
                line.clear();
            }
        }
        reader.consume(chunk);
        drained += chunk;
        if drained > FLOOD_LIMIT {
            return Ok(LineOutcome::Flooded);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    async fn read_all(input: &[u8]) -> Vec<LineOutcome> {
        let mut reader = BufReader::new(input);
        let mut outcomes = Vec::new();
        loop {
            let outcome = read_line(&mut reader).await.unwrap();
            let is_eof = outcome == LineOutcome::Eof;
            outcomes.push(outcome);
            if is_eof {
                return outcomes;
            }
        }
    }

    #[tokio::test]
    async fn crlf_line_is_returned_without_terminator() {
        let outcomes = read_all(b"EHLO client.example\r\n").await;
        assert_eq!(
            outcomes[0],
            LineOutcome::Line("EHLO client.example".to_owned())
        );
    }

    #[tokio::test]
    async fn bare_lf_is_rejected() {
        let outcomes = read_all(b"EHLO client.example\n").await;
        assert_eq!(outcomes[0], LineOutcome::BadLineEnding);
    }

    #[tokio::test]
    async fn stray_cr_inside_line_is_rejected() {
        let outcomes = read_all(b"EHLO cli\rent\r\n").await;
        assert_eq!(outcomes[0], LineOutcome::BadLineEnding);
    }

    #[tokio::test]
    async fn non_ascii_is_rejected() {
        let outcomes = read_all("EHLO müller.example\r\n".as_bytes()).await;
        assert_eq!(outcomes[0], LineOutcome::NotAscii);
    }

    #[tokio::test]
    async fn over_limit_line_is_too_long_and_stream_recovers() {
        // 600 octets of verb, then a valid QUIT on the same stream:
        // the long line must be drained, not left to corrupt parsing.
        let mut input = vec![b'X'; 600];
        input.extend_from_slice(b"\r\nQUIT\r\n");
        let outcomes = read_all(&input).await;
        assert_eq!(outcomes[0], LineOutcome::TooLong);
        assert_eq!(outcomes[1], LineOutcome::Line("QUIT".to_owned()));
    }

    #[tokio::test]
    async fn unterminated_input_is_eof_not_a_line() {
        let outcomes = read_all(b"EHLO client.example").await;
        assert_eq!(outcomes[0], LineOutcome::Eof);
    }
}
