//! Outbound IMAP client for the import wizard: pull a user's existing
//! mail from Gmail/Outlook/any IMAP host into their alo Inbox.
//!
//! Two layers, split so the fiddly protocol is testable without TLS or a
//! network:
//! - [`fetch_inbox`] speaks IMAP over any async stream (LOGIN → SELECT
//!   INBOX → FETCH the most-recent N `BODY.PEEK[]` → LOGOUT), handling
//!   literals by exact byte count. A plaintext mock exercises it.
//! - [`import`] resolves the host, refuses any non-public address
//!   (SSRF — the user names the host), pins the verified IP, opens
//!   **verified** implicit TLS (real Mozilla roots — the user's
//!   password is on this wire), runs `fetch_inbox`, and ingests each
//!   message into the Inbox, skipping any whose `Message-ID` is already
//!   present (idempotent re-import).
//!
//! Scope (recorded in `docs/interop.md`): INBOX only, the most-recent
//! [`MAX_MESSAGES`], done synchronously. Other folders, full-mailbox
//! migration, and background/resume are follow-ups — this is the "bring
//! your recent mail" onboarding slice. The password is never logged.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use alo_ai::egress::is_blocked_ip;
use alo_store::AccountStore;
use mail_parser::MessageParser;
use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

/// The most-recent messages pulled in one import (a bounded, synchronous
/// operation; full migration is a follow-up).
pub const MAX_MESSAGES: u32 = 500;
/// Largest single message accepted from the remote server.
const MAX_MESSAGE_BYTES: usize = 40 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const IO_TIMEOUT: Duration = Duration::from_secs(120);

/// What an import attempt did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct ImportOutcome {
    /// Newly-stored messages.
    pub imported: u32,
    /// Messages already present (by `Message-ID`) and skipped.
    pub skipped: u32,
    /// Messages that failed to store (logged, not fatal to the batch).
    pub failed: u32,
}

/// Why an import could not run.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// The host did not resolve, or resolved only to blocked (private/
    /// loopback/link-local) addresses.
    #[error("the mail server address is invalid or not reachable")]
    Host,
    /// TCP/TLS transport failure.
    #[error("could not connect securely to the mail server")]
    Connect,
    /// The IMAP server rejected the credentials.
    #[error("the username or password was not accepted")]
    Auth,
    /// A protocol or I/O error mid-session.
    #[error("the mail server did not respond as expected")]
    Protocol,
}

/// Connection details the user supplies.
pub struct ImapConfig<'a> {
    pub host: &'a str,
    pub port: u16,
    pub username: &'a str,
    pub password: &'a str,
}

/// Resolves + SSRF-guards `config.host`, connects over verified TLS,
/// fetches the recent INBOX, and ingests into `acc`'s Inbox with
/// `Message-ID` dedup.
pub async fn import(
    acc: &AccountStore,
    config: &ImapConfig<'_>,
) -> Result<ImportOutcome, ImportError> {
    let addr = resolve_public(config.host, config.port).await?;
    let server_name =
        ServerName::try_from(config.host.to_owned()).map_err(|_| ImportError::Host)?;

    let tcp = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr))
        .await
        .map_err(|_| ImportError::Connect)?
        .map_err(|_| ImportError::Connect)?;
    let connector = TlsConnector::from(Arc::new(tls_config()));
    let tls = tokio::time::timeout(CONNECT_TIMEOUT, connector.connect(server_name, tcp))
        .await
        .map_err(|_| ImportError::Connect)?
        .map_err(|_| ImportError::Connect)?;

    let messages = fetch_inbox(tls, config.username, config.password, MAX_MESSAGES).await?;
    import_messages(acc, messages).await
}

/// Resolves `host:port` to a single **public** socket address, refusing
/// the host if it does not resolve or any resolved address is blocked
/// (loopback/private/link-local/…). Pinning the checked IP for the
/// caller's connect closes the DNS-rebind gap.
async fn resolve_public(host: &str, port: u16) -> Result<std::net::SocketAddr, ImportError> {
    let addrs: Vec<std::net::SocketAddr> = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| ImportError::Host)?
        .collect();
    if addrs.is_empty() {
        return Err(ImportError::Host);
    }
    // Refuse if ANY resolved address is blocked — a host that resolves to
    // both a public and an internal address is not trustworthy.
    if addrs.iter().any(|a| is_blocked_ip(a.ip())) {
        return Err(ImportError::Host);
    }
    Ok(addrs[0])
}

/// A rustls client config that verifies the server certificate against
/// the bundled Mozilla roots (the user's password is on this wire — an
/// accept-any verifier would invite a MITM).
fn tls_config() -> rustls::ClientConfig {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth()
}

/// Runs the IMAP session over `stream` and returns the raw bytes of the
/// most-recent `max` INBOX messages. Generic over the stream so the
/// protocol is unit-tested against a plaintext mock.
pub async fn fetch_inbox<S>(
    stream: S,
    username: &str,
    password: &str,
    max: u32,
) -> Result<Vec<Vec<u8>>, ImportError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut conn = ImapConn::new(stream);
    conn.read_greeting().await?;
    conn.login(username, password).await?;
    let exists = conn.select_inbox().await?;
    let messages = if exists == 0 {
        Vec::new()
    } else {
        let lo = exists.saturating_sub(max).saturating_add(1).max(1);
        conn.fetch_bodies(lo, exists).await?
    };
    conn.logout().await;
    Ok(messages)
}

/// Ingests fetched messages into the account's Inbox, skipping any whose
/// `Message-ID` is already stored (idempotent re-import). Public so the
/// dedup/ingest half can be tested without a live IMAP server.
pub async fn import_messages(
    acc: &AccountStore,
    messages: Vec<Vec<u8>>,
) -> Result<ImportOutcome, ImportError> {
    let inbox = acc.inbox().await.map_err(|_| ImportError::Protocol)?;

    // Parse each message's Message-ID once, then ask the store which are
    // already present (one query, not one per message).
    let ids: Vec<Option<String>> = messages.iter().map(|raw| message_id(raw)).collect();
    let present: HashSet<String> = {
        let known: Vec<String> = ids.iter().flatten().cloned().collect();
        acc.existing_message_ids(&known)
            .await
            .map_err(|_| ImportError::Protocol)?
    };

    let mut out = ImportOutcome {
        imported: 0,
        skipped: 0,
        failed: 0,
    };
    for (raw, id) in messages.iter().zip(ids.iter()) {
        if let Some(id) = id
            && present.contains(id)
        {
            out.skipped += 1;
            continue;
        }
        match acc.ingest(&inbox, raw).await {
            Ok(_) => out.imported += 1,
            Err(error) => {
                tracing::warn!(%error, "imap import: one message failed to store");
                out.failed += 1;
            }
        }
    }
    Ok(out)
}

/// Extracts the `Message-ID` in the **bracketed** form the store keeps
/// (`<id@host>`), so the dedup query matches `messages.message_id_hdr`.
/// mail-parser returns the id without brackets, so we re-add them.
fn message_id(raw: &[u8]) -> Option<String> {
    let parsed = MessageParser::default().parse(raw)?;
    let bare = parsed
        .message_id()?
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .to_owned();
    (!bare.is_empty()).then(|| format!("<{bare}>"))
}

/// A buffered IMAP connection with a monotonically increasing command tag.
struct ImapConn<S> {
    stream: BufReader<S>,
    tag: u32,
}

impl<S> ImapConn<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn new(stream: S) -> Self {
        Self {
            stream: BufReader::new(stream),
            tag: 0,
        }
    }

    async fn read_greeting(&mut self) -> Result<(), ImportError> {
        let line = self.read_line().await?;
        // `* OK ...` — anything else (e.g. `* BYE`) is a refusal.
        if line.starts_with("* OK") {
            Ok(())
        } else {
            Err(ImportError::Protocol)
        }
    }

    async fn login(&mut self, user: &str, pass: &str) -> Result<(), ImportError> {
        let tag = self.next_tag();
        let cmd = format!("{tag} LOGIN {} {}\r\n", quote(user), quote(pass));
        self.write(cmd.as_bytes()).await?;
        // Consume untagged lines until our tagged completion.
        match self.read_completion(&tag).await? {
            Completion::Ok => Ok(()),
            Completion::No => Err(ImportError::Auth),
            Completion::Bad => Err(ImportError::Protocol),
        }
    }

    /// SELECTs INBOX and returns its message count (`* n EXISTS`).
    async fn select_inbox(&mut self) -> Result<u32, ImportError> {
        let tag = self.next_tag();
        self.write(format!("{tag} SELECT INBOX\r\n").as_bytes())
            .await?;
        let mut exists = 0u32;
        loop {
            let line = self.read_line().await?;
            if let Some(rest) = tagged(&line, &tag) {
                return if rest.starts_with("OK") {
                    Ok(exists)
                } else {
                    Err(ImportError::Protocol)
                };
            }
            // `* <n> EXISTS`
            if let Some(n) = line
                .strip_prefix("* ")
                .and_then(|r| r.strip_suffix(" EXISTS"))
            {
                exists = n.trim().parse().unwrap_or(exists);
            }
        }
    }

    /// FETCHes `lo:hi BODY.PEEK[]`, returning each message's raw bytes.
    async fn fetch_bodies(&mut self, lo: u32, hi: u32) -> Result<Vec<Vec<u8>>, ImportError> {
        let tag = self.next_tag();
        self.write(format!("{tag} FETCH {lo}:{hi} BODY.PEEK[]\r\n").as_bytes())
            .await?;
        let mut messages = Vec::new();
        loop {
            let line = self.read_line().await?;
            if let Some(rest) = tagged(&line, &tag) {
                return if rest.starts_with("OK") {
                    Ok(messages)
                } else {
                    Err(ImportError::Protocol)
                };
            }
            // A FETCH response line ends with a literal `{size}` for BODY[].
            if let Some(size) = literal_size(&line) {
                if size > MAX_MESSAGE_BYTES {
                    return Err(ImportError::Protocol);
                }
                let mut body = vec![0u8; size];
                tokio::time::timeout(IO_TIMEOUT, self.stream.read_exact(&mut body))
                    .await
                    .map_err(|_| ImportError::Protocol)?
                    .map_err(|_| ImportError::Protocol)?;
                messages.push(body);
                // The literal is followed by the rest of the response line
                // (usually `)`), then CRLF — consume it.
                let _trailer = self.read_line().await?;
            }
        }
    }

    async fn logout(&mut self) {
        let tag = self.next_tag();
        // Best-effort; the session is done regardless.
        let _ = self.write(format!("{tag} LOGOUT\r\n").as_bytes()).await;
    }

    /// Reads untagged lines until the tagged completion, returning its
    /// status. Used where untagged content is irrelevant (LOGIN).
    async fn read_completion(&mut self, tag: &str) -> Result<Completion, ImportError> {
        loop {
            let line = self.read_line().await?;
            if let Some(rest) = tagged(&line, tag) {
                return Ok(if rest.starts_with("OK") {
                    Completion::Ok
                } else if rest.starts_with("NO") {
                    Completion::No
                } else {
                    Completion::Bad
                });
            }
        }
    }

    fn next_tag(&mut self) -> String {
        self.tag += 1;
        format!("a{}", self.tag)
    }

    async fn write(&mut self, bytes: &[u8]) -> Result<(), ImportError> {
        tokio::time::timeout(IO_TIMEOUT, self.stream.get_mut().write_all(bytes))
            .await
            .map_err(|_| ImportError::Protocol)?
            .map_err(|_| ImportError::Protocol)?;
        Ok(())
    }

    /// Reads one CRLF-terminated protocol line (without the CRLF).
    /// Bounded so a hostile server cannot stream an unbounded line.
    async fn read_line(&mut self) -> Result<String, ImportError> {
        let mut buf = Vec::new();
        loop {
            let mut byte = [0u8; 1];
            let n = tokio::time::timeout(IO_TIMEOUT, self.stream.read_exact(&mut byte))
                .await
                .map_err(|_| ImportError::Protocol)?
                .map_err(|_| ImportError::Protocol)?;
            let _ = n;
            if byte[0] == b'\n' {
                if buf.last() == Some(&b'\r') {
                    buf.pop();
                }
                break;
            }
            buf.push(byte[0]);
            if buf.len() > 64 * 1024 {
                return Err(ImportError::Protocol);
            }
        }
        String::from_utf8(buf).map_err(|_| ImportError::Protocol)
    }
}

enum Completion {
    Ok,
    No,
    Bad,
}

/// If `line` is our tagged completion (`<tag> ...`), returns the rest.
fn tagged<'a>(line: &'a str, tag: &str) -> Option<&'a str> {
    line.strip_prefix(tag)
        .and_then(|r| r.strip_prefix(' '))
        .map(str::trim_start)
}

/// The trailing IMAP literal size `{n}` on a response line, if present.
fn literal_size(line: &str) -> Option<usize> {
    let inner = line.trim_end().strip_suffix('}')?;
    let brace = inner.rfind('{')?;
    inner[brace + 1..].parse().ok()
}

/// Quotes a string as an IMAP quoted-string (RFC 3501 §4.3), escaping
/// `\` and `"`. Refuses CR/LF (they would break the command line — such
/// a value simply cannot be a valid credential).
fn quote(s: &str) -> String {
    let escaped: String = s
        .chars()
        .filter(|c| *c != '\r' && *c != '\n')
        .flat_map(|c| match c {
            '\\' => vec!['\\', '\\'],
            '"' => vec!['\\', '"'],
            c => vec![c],
        })
        .collect();
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn literal_size_parsing() {
        assert_eq!(literal_size("* 1 FETCH (BODY[] {2748}"), Some(2748));
        assert_eq!(literal_size("* 1 FETCH (UID 5 BODY[] {12}\r\n"), Some(12));
        assert_eq!(literal_size("a3 OK done"), None);
        assert_eq!(literal_size("* 2 EXISTS"), None);
    }

    #[test]
    fn quoting_escapes_and_strips_crlf() {
        assert_eq!(quote("user"), "\"user\"");
        assert_eq!(quote("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(quote("x\r\nA LOGIN evil"), "\"xA LOGIN evil\"");
    }

    #[test]
    fn tagged_completion() {
        assert_eq!(tagged("a1 OK LOGIN done", "a1"), Some("OK LOGIN done"));
        assert_eq!(tagged("* 1 EXISTS", "a1"), None);
        assert_eq!(tagged("a10 OK", "a1"), None); // not a prefix-false-match
    }

    #[test]
    fn message_id_extraction() {
        // Bracketed form, matching what the store keeps in message_id_hdr.
        let raw = b"Subject: hi\r\nMessage-ID: <abc@x.eu>\r\n\r\nbody\r\n";
        assert_eq!(message_id(raw).as_deref(), Some("<abc@x.eu>"));
        assert_eq!(message_id(b"Subject: none\r\n\r\nx"), None);
    }

    /// A scripted mock IMAP server over an in-memory duplex stream: greeting
    /// → LOGIN ok → SELECT with 3 EXISTS → FETCH returns 2 messages (the
    /// most-recent 2 of 3) → LOGOUT.
    #[tokio::test]
    async fn fetch_inbox_protocol_over_a_mock() {
        let (client, mut server) = tokio::io::duplex(64 * 1024);
        let mock = tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            async fn line(s: &mut tokio::io::DuplexStream) -> String {
                let mut buf = Vec::new();
                loop {
                    let mut b = [0u8; 1];
                    if s.read_exact(&mut b).await.is_err() {
                        break;
                    }
                    if b[0] == b'\n' {
                        break;
                    }
                    if b[0] != b'\r' {
                        buf.push(b[0]);
                    }
                }
                String::from_utf8_lossy(&buf).into_owned()
            }
            server.write_all(b"* OK mock IMAP ready\r\n").await.unwrap();
            let login = line(&mut server).await;
            assert!(login.contains("LOGIN"), "{login}");
            server
                .write_all(b"a1 OK LOGIN completed\r\n")
                .await
                .unwrap();
            let select = line(&mut server).await;
            assert!(select.contains("SELECT INBOX"), "{select}");
            server
                .write_all(b"* 3 EXISTS\r\n* 0 RECENT\r\na2 OK [READ-WRITE] SELECT\r\n")
                .await
                .unwrap();
            let fetch = line(&mut server).await;
            assert!(fetch.contains("FETCH 2:3 BODY.PEEK[]"), "{fetch}");
            // Two messages returned as literals.
            let m2 = b"Subject: two\r\nMessage-ID: <2@x>\r\n\r\nsecond\r\n";
            let m3 = b"Subject: three\r\nMessage-ID: <3@x>\r\n\r\nthird\r\n";
            server
                .write_all(format!("* 2 FETCH (BODY[] {{{}}}\r\n", m2.len()).as_bytes())
                .await
                .unwrap();
            server.write_all(m2).await.unwrap();
            server.write_all(b")\r\n").await.unwrap();
            server
                .write_all(format!("* 3 FETCH (BODY[] {{{}}}\r\n", m3.len()).as_bytes())
                .await
                .unwrap();
            server.write_all(m3).await.unwrap();
            server.write_all(b")\r\n").await.unwrap();
            server
                .write_all(b"a3 OK FETCH completed\r\n")
                .await
                .unwrap();
            // LOGOUT is best-effort on the client (it doesn't read the
            // reply and drops the stream), so tolerate a closed pipe here.
            let _logout = line(&mut server).await;
            let _ = server.write_all(b"a4 OK LOGOUT\r\n").await;
        });

        let messages = fetch_inbox(client, "me@x.eu", "pw", 2).await.unwrap();
        mock.await.unwrap();
        assert_eq!(messages.len(), 2);
        assert!(String::from_utf8_lossy(&messages[0]).contains("second"));
        assert!(String::from_utf8_lossy(&messages[1]).contains("<3@x>"));
    }

    #[tokio::test]
    async fn login_failure_maps_to_auth_error() {
        let (client, mut server) = tokio::io::duplex(4096);
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            server.write_all(b"* OK ready\r\n").await.unwrap();
            let mut buf = [0u8; 256];
            let _ = server.read(&mut buf).await;
            server
                .write_all(b"a1 NO [AUTHENTICATIONFAILED] bad\r\n")
                .await
                .unwrap();
        });
        let err = fetch_inbox(client, "u", "wrong", 10).await.unwrap_err();
        assert!(matches!(err, ImportError::Auth), "{err:?}");
    }
}
