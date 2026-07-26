//! TCP transport: the listener, per-connection wire I/O, and the
//! read-side limits and timeouts RFC 5321 requires.
//!
//! All protocol decisions live in [`crate::session`]; DATA content
//! collection lives in [`crate::data`]; this module moves bytes
//! safely and stitches the two together around the spool.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tracing::Instrument;

use crate::config::SmtpConfig;
use crate::data::{self, DataError};
use crate::envelope::Envelope;
use crate::error::SmtpError;
use crate::line::{RawLine, read_raw_line};
use crate::received;
use crate::reply::Reply;
use crate::session::{Action, Session};
use crate::spool::Spool;

/// Command line limit: 512 octets including CRLF (RFC 5321
/// §4.5.3.1.4; no extension we advertise raises it). Enforced during
/// read, never after buffering (protocol skill non-negotiable).
const MAX_COMMAND_LINE: usize = 512;

/// Hard ceiling on octets drained while hunting for a line ending on
/// the command channel; beyond it the peer is flooding.
const FLOOD_LIMIT: usize = 64 * 1024;

/// Idle timeout while awaiting a command, RFC 5321 §4.5.3.2.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(300);

/// Overall budget for receiving one message body. Deliberately
/// stricter than the per-wait timeouts of §4.5.3.2 (a total budget,
/// not per-block) as anti-flood policy — recorded in docs/interop.md
/// "Standing policies".
const DATA_TIMEOUT: Duration = Duration::from_secs(600);

/// Budget for writing one reply: a peer that stops reading (full
/// receive window) must not pin this task forever — the read-side
/// timeouts never fire while a write pends.
const WRITE_TIMEOUT: Duration = Duration::from_secs(30);

/// Delay before re-accepting after a transient `accept()` error, so a
/// resource-exhaustion condition cannot spin the accept loop hot.
const ACCEPT_RETRY_DELAY: Duration = Duration::from_millis(100);

/// Binds the configured address, opens the spool, and serves forever.
///
/// # Errors
/// [`SmtpError::Bind`] when the listener cannot bind,
/// [`SmtpError::Spool`] when the spool root cannot be prepared; once
/// running, per-connection failures are logged and never fatal.
pub async fn run(config: SmtpConfig) -> Result<(), SmtpError> {
    let spool = Spool::new(&config.spool_dir).map_err(|source| SmtpError::Spool {
        path: config.spool_dir.display().to_string(),
        source,
    })?;
    let listener = TcpListener::bind(config.bind_addr)
        .await
        .map_err(|source| SmtpError::Bind {
            addr: config.bind_addr,
            source,
        })?;
    tracing::info!(
        addr = %config.bind_addr,
        hostname = %config.hostname,
        spool = %config.spool_dir.display(),
        "ficina-smtp listening"
    );
    serve(listener, Arc::new(config), Arc::new(spool)).await
}

/// Accept loop over an already-bound listener (also the seam the
/// integration tests use to serve on an ephemeral port).
///
/// # Errors
/// Never returns an error today; the `Result` is the stable signature
/// for when shutdown handling lands.
pub async fn serve(
    listener: TcpListener,
    config: Arc<SmtpConfig>,
    spool: Arc<Spool>,
) -> Result<(), SmtpError> {
    // Bounds concurrent sessions; connection #cap+1 gets 421 + close
    // instead of an unbounded task (review finding: remote task-pinning).
    let limiter = Arc::new(tokio::sync::Semaphore::new(config.max_connections));
    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                let span = tracing::info_span!("smtp_session", %peer);
                match Arc::clone(&limiter).try_acquire_owned() {
                    Ok(permit) => {
                        let config = Arc::clone(&config);
                        let spool = Arc::clone(&spool);
                        tokio::spawn(
                            async move {
                                let _permit = permit;
                                if let Err(error) =
                                    handle_connection(stream, peer, config, spool).await
                                {
                                    // Peer-side I/O failures are normal
                                    // churn, not service errors.
                                    tracing::debug!(%error, "session ended with I/O error");
                                }
                            }
                            .instrument(span),
                        );
                    }
                    Err(_no_permit) => {
                        let hostname = config.hostname.clone();
                        tokio::spawn(
                            async move {
                                tracing::warn!("connection limit reached; refusing with 421");
                                let (_read_half, mut write_half) = stream.into_split();
                                let _best_effort = write_reply(
                                    &mut write_half,
                                    &Reply::service_closing(&hostname),
                                )
                                .await;
                            }
                            .instrument(span),
                        );
                    }
                }
            }
            Err(error) => {
                tracing::warn!(%error, "accept failed; retrying");
                tokio::time::sleep(ACCEPT_RETRY_DELAY).await;
            }
        }
    }
}

/// Drives one connection: greeting, then command/reply/DATA until
/// close.
async fn handle_connection(
    stream: TcpStream,
    peer: SocketAddr,
    config: Arc<SmtpConfig>,
    spool: Arc<Spool>,
) -> std::io::Result<()> {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut session = Session::new(config.hostname.clone(), config.max_rcpt);

    tracing::info!("session opened");
    write_reply(&mut write_half, &session.greeting()).await?;

    loop {
        let outcome = match tokio::time::timeout(
            COMMAND_TIMEOUT,
            read_raw_line(&mut reader, MAX_COMMAND_LINE, FLOOD_LIMIT),
        )
        .await
        {
            Ok(result) => result?,
            Err(_elapsed) => {
                // RFC 5321 §4.5.3.2: on timeout, close with 421.
                tracing::info!("idle timeout; closing");
                write_reply(&mut write_half, &Reply::service_closing(&config.hostname)).await?;
                break;
            }
        };

        match outcome {
            RawLine::Line(bytes) => {
                // Commands are ASCII (§2.4); SMTPUTF8 (RFC 6531) is a
                // later-milestone extension and applies to addresses,
                // not to reaching this check with arbitrary bytes.
                if !bytes.is_ascii() {
                    write_reply(&mut write_half, &Reply::command_unrecognized()).await?;
                    continue;
                }
                // ASCII just verified, so this conversion is lossless.
                let line = String::from_utf8_lossy(&bytes).into_owned();
                let (reply, action) = session.on_line(&line);
                write_reply(&mut write_half, &reply).await?;
                match action {
                    Action::Continue => {}
                    Action::Close => {
                        tracing::info!("session closed by QUIT");
                        break;
                    }
                    Action::EnterData => {
                        let keep_going = handle_data_phase(
                            &mut reader,
                            &mut write_half,
                            &mut session,
                            peer,
                            &config,
                            &spool,
                        )
                        .await?;
                        if !keep_going {
                            break;
                        }
                    }
                }
            }
            RawLine::TooLong { .. } => {
                write_reply(&mut write_half, &Reply::line_too_long()).await?;
            }
            RawLine::BadEol => {
                write_reply(&mut write_half, &Reply::bare_line_ending()).await?;
            }
            RawLine::Flooded => {
                tracing::warn!("peer flooded without newline; closing");
                write_reply(&mut write_half, &Reply::service_closing(&config.hostname)).await?;
                break;
            }
            RawLine::Eof => {
                tracing::info!("peer disconnected");
                break;
            }
        }
    }
    Ok(())
}

/// Collects one message after 354, stamps `Received:`, and spools it
/// durably. Returns whether the session may continue.
async fn handle_data_phase(
    reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: &mut OwnedWriteHalf,
    session: &mut Session,
    peer: SocketAddr,
    config: &Arc<SmtpConfig>,
    spool: &Arc<Spool>,
) -> std::io::Result<bool> {
    let collected = match tokio::time::timeout(
        DATA_TIMEOUT,
        data::read_message(reader, config.max_message_size),
    )
    .await
    {
        Ok(result) => result,
        Err(_elapsed) => {
            tracing::info!("DATA timeout; closing");
            session.end_data();
            write_reply(writer, &Reply::service_closing(&config.hostname)).await?;
            return Ok(false);
        }
    };

    match collected {
        Ok(body) => {
            // EnterData is only reachable from a live transaction; if
            // that invariant ever breaks, fail loudly (451) rather
            // than spool a fabricated envelope into the M2 contract.
            let Some((mail_from, rcpt_to)) = session.envelope_fields() else {
                tracing::error!("DATA completed without a transaction — invariant broken");
                session.end_data();
                write_reply(writer, &Reply::local_error()).await?;
                return Ok(true);
            };
            let envelope = Envelope {
                helo: session.helo_client().to_owned(),
                peer: peer.to_string(),
                mail_from,
                rcpt_to,
                received_at: jiff::Timestamp::now().to_string(),
            };
            let id = spool.next_id();
            let now = jiff::Timestamp::now().to_zoned(jiff::tz::TimeZone::UTC);
            let header = received::stamp(
                session.helo_client(),
                &peer.ip().to_string(),
                &config.hostname,
                session.protocol_name(),
                &id,
                &now,
            );
            let mut message = Vec::with_capacity(header.len() + body.len());
            message.extend_from_slice(header.as_bytes());
            message.extend_from_slice(&body);

            // Spool I/O is synchronous by design (fsync durability);
            // run it off the reactor.
            let spool_task = {
                let spool = Arc::clone(spool);
                let id = id.clone();
                tokio::task::spawn_blocking(move || spool.store(&id, &envelope, &message))
            };
            session.end_data();
            match spool_task.await {
                Ok(Ok(())) => {
                    tracing::info!(%id, size = body.len(), "message accepted");
                    write_reply(writer, &Reply::ok_queued(&id)).await?;
                }
                Ok(Err(error)) => {
                    // Durability failed: the message was NOT accepted;
                    // 451 tells the client to retry (RFC 5321 §4.2.4).
                    tracing::error!(%error, "spool write failed");
                    write_reply(writer, &Reply::local_error()).await?;
                }
                Err(join_error) => {
                    tracing::error!(%join_error, "spool task panicked");
                    write_reply(writer, &Reply::local_error()).await?;
                }
            }
            Ok(true)
        }
        Err(DataError::TooLarge) => {
            session.end_data();
            write_reply(writer, &Reply::message_too_large()).await?;
            Ok(true)
        }
        Err(DataError::LineTooLong) => {
            session.end_data();
            write_reply(writer, &Reply::line_too_long()).await?;
            Ok(true)
        }
        Err(DataError::BareLineEnding) => {
            // Smuggling shape inside content: reject AND close — the
            // stream beyond this point cannot be trusted to re-sync.
            tracing::warn!("bare line ending inside DATA; closing");
            session.end_data();
            write_reply(writer, &Reply::bare_line_ending()).await?;
            Ok(false)
        }
        Err(DataError::Flooded) => {
            tracing::warn!("peer flooded the DATA channel; closing");
            session.end_data();
            write_reply(writer, &Reply::service_closing(&config.hostname)).await?;
            Ok(false)
        }
        Err(DataError::UnexpectedEof) => {
            tracing::info!("peer disconnected mid-DATA; message discarded");
            session.end_data();
            Ok(false)
        }
        Err(DataError::Io(error)) => {
            session.end_data();
            Err(error)
        }
    }
}

async fn write_reply(writer: &mut OwnedWriteHalf, reply: &Reply) -> std::io::Result<()> {
    tokio::time::timeout(WRITE_TIMEOUT, async {
        writer.write_all(reply.to_string().as_bytes()).await?;
        writer.flush().await
    })
    .await
    .map_err(|_elapsed| {
        std::io::Error::new(std::io::ErrorKind::TimedOut, "reply write timed out")
    })?
}
