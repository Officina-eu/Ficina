//! Runtime configuration for the SMTP service, read from environment.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use crate::error::SmtpError;

/// Environment variable naming the socket address to listen on.
pub const ENV_ADDR: &str = "FICINA_SMTP_ADDR";
/// Environment variable naming the hostname used in banners/replies.
pub const ENV_HOSTNAME: &str = "FICINA_SMTP_HOSTNAME";
/// Environment variable naming the spool directory.
pub const ENV_SPOOL_DIR: &str = "FICINA_SMTP_SPOOL_DIR";
/// Environment variable for the maximum message size in octets.
pub const ENV_MAX_MESSAGE_SIZE: &str = "FICINA_SMTP_MAX_MESSAGE_SIZE";
/// Environment variable for the per-transaction recipient limit.
pub const ENV_MAX_RCPT: &str = "FICINA_SMTP_MAX_RCPT";
/// Environment variable for the concurrent-connection cap.
pub const ENV_MAX_CONNECTIONS: &str = "FICINA_SMTP_MAX_CONNECTIONS";
/// Environment flag enabling outbound delivery (off by default — see
/// the relay-safety note on [`OutboundConfig`]).
pub const ENV_OUTBOUND_ENABLED: &str = "FICINA_SMTP_OUTBOUND_ENABLED";
/// Environment variable routing all outbound mail to one host.
pub const ENV_SMARTHOST: &str = "FICINA_SMTP_SMARTHOST";
/// Environment variable for the retry base delay in seconds.
pub const ENV_RETRY_BASE_SECS: &str = "FICINA_SMTP_RETRY_BASE_SECS";
/// Environment variable for the retry cap in seconds.
pub const ENV_RETRY_CAP_SECS: &str = "FICINA_SMTP_RETRY_CAP_SECS";
/// Environment variable for the maximum delivery attempts.
pub const ENV_MAX_ATTEMPTS: &str = "FICINA_SMTP_MAX_ATTEMPTS";
/// Environment variable for the queue polling interval in seconds.
pub const ENV_QUEUE_INTERVAL_SECS: &str = "FICINA_SMTP_QUEUE_INTERVAL_SECS";
/// Environment variable for the submission (STARTTLS) listener address.
pub const ENV_SUBMISSION_ADDR: &str = "FICINA_SMTP_SUBMISSION_ADDR";
/// Environment variable for the implicit-TLS submission listener address.
pub const ENV_IMPLICIT_TLS_ADDR: &str = "FICINA_SMTP_IMPLICIT_TLS_ADDR";
/// Environment variable for the TLS certificate PEM path.
pub const ENV_TLS_CERT: &str = "FICINA_SMTP_TLS_CERT";
/// Environment variable for the TLS private-key PEM path.
pub const ENV_TLS_KEY: &str = "FICINA_SMTP_TLS_KEY";
/// Environment variable for the submission credentials file (dev
/// bootstrap for AUTH; replaced by ficina-identity in M9). One
/// `username:password` per line.
pub const ENV_CREDENTIALS_FILE: &str = "FICINA_SMTP_CREDENTIALS_FILE";
/// Environment variable listing the domains this server hosts
/// (comma-separated). The MX anti-open-relay guard: only these
/// domains' recipients are accepted on port 25.
pub const ENV_LOCAL_DOMAINS: &str = "FICINA_SMTP_LOCAL_DOMAINS";
/// Environment flag permitting self-signed certificate generation when
/// no PEM is configured (development only).
pub const ENV_ALLOW_SELF_SIGNED: &str = "FICINA_SMTP_ALLOW_SELF_SIGNED";

const DEFAULT_ADDR: &str = "0.0.0.0:2525";
const DEFAULT_HOSTNAME: &str = "ficina.test";
const DEFAULT_SPOOL_DIR: &str = "./spool";
/// 25 MiB default, in line with common provider limits.
const DEFAULT_MAX_MESSAGE_SIZE: usize = 25 * 1024 * 1024;
/// RFC 5321 §4.5.3.1.8: servers MUST accept at least 100 recipients.
const DEFAULT_MAX_RCPT: usize = 100;
const MIN_MAX_RCPT: usize = 100;
/// RFC 5321 §4.5.3.1.7: servers MUST accept messages of at least
/// 64K octets — a lower configured cap ships a non-compliant server.
const MIN_MESSAGE_SIZE: usize = 64 * 1024;
const DEFAULT_MAX_CONNECTIONS: usize = 256;
const DEFAULT_RETRY_BASE_SECS: u64 = 60;
const DEFAULT_RETRY_CAP_SECS: u64 = 3600;
const DEFAULT_MAX_ATTEMPTS: u32 = 8;
const DEFAULT_QUEUE_INTERVAL_SECS: u64 = 30;

// Compile-time guarantees that the defaults never drift below the
// RFC 5321 floors (§4.5.3.1.8 recipients, §4.5.3.1.7 message size).
const _: () = assert!(DEFAULT_MAX_RCPT >= MIN_MAX_RCPT);
const _: () = assert!(DEFAULT_MAX_MESSAGE_SIZE >= MIN_MESSAGE_SIZE);
const _: () = assert!(DEFAULT_MAX_CONNECTIONS >= 1);

/// Validated service configuration.
#[derive(Debug, Clone)]
pub struct SmtpConfig {
    /// Address the listener binds to.
    pub bind_addr: SocketAddr,
    /// Hostname announced in the 220 greeting and EHLO reply
    /// (RFC 5321 §4.1.1.1: the fully-qualified domain of the server).
    pub hostname: String,
    /// Root of the durable message spool.
    pub spool_dir: PathBuf,
    /// Maximum accepted message size in octets, enforced during read.
    pub max_message_size: usize,
    /// Maximum recipients per transaction (≥ 100 per §4.5.3.1.8).
    pub max_rcpt: usize,
    /// Concurrent-connection cap; excess connections are greeted with
    /// 421 and closed so one host cannot pin unlimited tasks.
    pub max_connections: usize,
    /// Outbound delivery settings; `None` means receive-only.
    pub outbound: Option<OutboundConfig>,
    /// Submission (STARTTLS, port 587) listener; `None` disables it.
    pub submission_addr: Option<SocketAddr>,
    /// Implicit-TLS submission (port 465) listener; `None` disables it.
    pub implicit_tls_addr: Option<SocketAddr>,
    /// TLS certificate + key PEM paths. `None` generates a self-signed
    /// certificate at startup (development only).
    pub tls: Option<TlsPaths>,
    /// Submission credentials file (dev AUTH bootstrap). `None` means
    /// no credentials — submission AUTH always fails (a safe default).
    pub credentials_file: Option<PathBuf>,
    /// Hosted domains (lowercased) for the MX anti-open-relay guard.
    /// Empty accepts all recipients (development); a non-empty list is
    /// required before outbound delivery may be enabled.
    pub local_domains: Vec<String>,
    /// Whether a self-signed certificate may be generated when no PEM
    /// is configured. Must be set explicitly so a production server
    /// never silently presents an untrusted cert (opportunistic-TLS
    /// MITM exposure).
    pub allow_self_signed: bool,
}

/// Paths to a TLS certificate chain and its private key (PEM).
#[derive(Debug, Clone)]
pub struct TlsPaths {
    /// Certificate chain PEM.
    pub cert: PathBuf,
    /// Private key PEM.
    pub key: PathBuf,
}

/// Outbound delivery configuration.
///
/// Relay safety: outbound is off unless [`ENV_OUTBOUND_ENABLED`] is
/// explicitly true, because M1 accepts any recipient and enabling
/// delivery without the AUTH gate (M3) would make an exposed instance
/// an open relay. A smarthost is the supported self-hosted route.
#[derive(Debug, Clone)]
pub struct OutboundConfig {
    /// Smarthost to relay all mail through; `None` means MX delivery.
    pub smarthost: Option<SocketAddr>,
    /// First-retry base delay.
    pub retry_base: std::time::Duration,
    /// Retry delay cap.
    pub retry_cap: std::time::Duration,
    /// Attempts before a transient failure bounces.
    pub max_attempts: u32,
    /// Queue polling interval.
    pub queue_interval: std::time::Duration,
}

impl SmtpConfig {
    /// Builds the configuration from environment variables, falling
    /// back to development defaults.
    ///
    /// # Errors
    /// Returns [`SmtpError::Config`] when a provided value cannot be
    /// used, with a message naming the variable and the expected form.
    pub fn from_env() -> Result<Self, SmtpError> {
        let addr_raw = std::env::var(ENV_ADDR).unwrap_or_else(|_| DEFAULT_ADDR.to_owned());
        let bind_addr: SocketAddr = addr_raw.parse().map_err(|_| SmtpError::Config {
            message: format!(
                "{ENV_ADDR}={addr_raw} is not a socket address; expected e.g. 0.0.0.0:2525"
            ),
        })?;

        let hostname = std::env::var(ENV_HOSTNAME).unwrap_or_else(|_| DEFAULT_HOSTNAME.to_owned());
        if hostname.is_empty() || hostname.contains(char::is_whitespace) {
            return Err(SmtpError::Config {
                message: format!(
                    "{ENV_HOSTNAME}={hostname:?} must be a non-empty hostname without whitespace"
                ),
            });
        }

        let spool_dir = PathBuf::from(
            std::env::var(ENV_SPOOL_DIR).unwrap_or_else(|_| DEFAULT_SPOOL_DIR.to_owned()),
        );

        let max_message_size = env_usize(ENV_MAX_MESSAGE_SIZE, DEFAULT_MAX_MESSAGE_SIZE)?;
        if max_message_size < MIN_MESSAGE_SIZE {
            return Err(SmtpError::Config {
                message: format!(
                    "{ENV_MAX_MESSAGE_SIZE} must be at least {MIN_MESSAGE_SIZE} octets"
                ),
            });
        }

        let max_rcpt = env_usize(ENV_MAX_RCPT, DEFAULT_MAX_RCPT)?;
        if max_rcpt < MIN_MAX_RCPT {
            // RFC 5321 §4.5.3.1.8 sets the floor; configuring below it
            // would ship a non-compliant server.
            return Err(SmtpError::Config {
                message: format!("{ENV_MAX_RCPT} must be at least {MIN_MAX_RCPT} (RFC 5321)"),
            });
        }

        let max_connections = env_usize(ENV_MAX_CONNECTIONS, DEFAULT_MAX_CONNECTIONS)?;
        if max_connections == 0 {
            return Err(SmtpError::Config {
                message: format!("{ENV_MAX_CONNECTIONS} must be at least 1"),
            });
        }

        let outbound = Self::outbound_from_env()?;

        let submission_addr = env_addr(ENV_SUBMISSION_ADDR)?;
        let implicit_tls_addr = env_addr(ENV_IMPLICIT_TLS_ADDR)?;

        let tls = match (std::env::var(ENV_TLS_CERT), std::env::var(ENV_TLS_KEY)) {
            (Ok(cert), Ok(key)) if !cert.is_empty() && !key.is_empty() => Some(TlsPaths {
                cert: PathBuf::from(cert),
                key: PathBuf::from(key),
            }),
            (Ok(one), Err(_)) | (Err(_), Ok(one)) if !one.is_empty() => {
                return Err(SmtpError::Config {
                    message: format!("{ENV_TLS_CERT} and {ENV_TLS_KEY} must be set together"),
                });
            }
            _ => None,
        };

        // Implicit-TLS (465) cannot run without a usable certificate;
        // a self-signed one is generated when none is configured, so
        // this never blocks dev — but warn nobody set a real cert.
        if implicit_tls_addr.is_some() && tls.is_none() {
            tracing::warn!(
                "implicit-TLS listener configured without a certificate; a self-signed one will be generated (development only)"
            );
        }

        let credentials_file = match std::env::var(ENV_CREDENTIALS_FILE) {
            Ok(path) if !path.is_empty() => Some(PathBuf::from(path)),
            _ => None,
        };

        let local_domains: Vec<String> = std::env::var(ENV_LOCAL_DOMAINS)
            .unwrap_or_default()
            .split(',')
            .map(|d| d.trim().to_ascii_lowercase())
            .filter(|d| !d.is_empty())
            .collect();

        // Anti-open-relay (security audit): outbound delivery must not
        // be enabled while the MX accepts recipients for any domain, or
        // an exposed instance relays to arbitrary externals. Enforced
        // in code, not left to the outbound-off default.
        if outbound.is_some() && local_domains.is_empty() {
            return Err(SmtpError::Config {
                message: format!(
                    "{ENV_OUTBOUND_ENABLED}=true requires {ENV_LOCAL_DOMAINS} to be set \
                     (the MX would otherwise be an open relay)"
                ),
            });
        }

        let allow_self_signed = env_bool(ENV_ALLOW_SELF_SIGNED)?;
        // A production server (real cert absent) must not silently
        // present a self-signed cert: require the explicit opt-in.
        if tls.is_none() && !allow_self_signed {
            return Err(SmtpError::Config {
                message: format!(
                    "no TLS certificate configured: set {ENV_TLS_CERT}/{ENV_TLS_KEY}, \
                     or {ENV_ALLOW_SELF_SIGNED}=true for development"
                ),
            });
        }

        Ok(Self {
            bind_addr,
            hostname,
            spool_dir,
            max_message_size,
            max_rcpt,
            max_connections,
            outbound,
            submission_addr,
            implicit_tls_addr,
            tls,
            credentials_file,
            local_domains,
            allow_self_signed,
        })
    }

    fn outbound_from_env() -> Result<Option<OutboundConfig>, SmtpError> {
        if !env_bool(ENV_OUTBOUND_ENABLED)? {
            return Ok(None);
        }
        let smarthost = match std::env::var(ENV_SMARTHOST) {
            Err(_) => None,
            Ok(raw) if raw.is_empty() => None,
            Ok(raw) => Some(raw.parse().map_err(|_| SmtpError::Config {
                message: format!("{ENV_SMARTHOST}={raw} is not a host:port address"),
            })?),
        };
        let retry_base =
            Duration::from_secs(env_u64(ENV_RETRY_BASE_SECS, DEFAULT_RETRY_BASE_SECS)?.max(1));
        let retry_cap = Duration::from_secs(
            env_u64(ENV_RETRY_CAP_SECS, DEFAULT_RETRY_CAP_SECS)?.max(retry_base.as_secs()),
        );
        let max_attempts =
            u32::try_from(env_u64(ENV_MAX_ATTEMPTS, u64::from(DEFAULT_MAX_ATTEMPTS))?)
                .unwrap_or(DEFAULT_MAX_ATTEMPTS)
                .max(1);
        let queue_interval = Duration::from_secs(
            env_u64(ENV_QUEUE_INTERVAL_SECS, DEFAULT_QUEUE_INTERVAL_SECS)?.max(1),
        );
        Ok(Some(OutboundConfig {
            smarthost,
            retry_base,
            retry_cap,
            max_attempts,
            queue_interval,
        }))
    }
}

fn env_bool(name: &str) -> Result<bool, SmtpError> {
    match std::env::var(name) {
        Err(_) => Ok(false),
        Ok(raw) => match raw.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" | "" => Ok(false),
            other => Err(SmtpError::Config {
                message: format!("{name}={other} must be a boolean (true/false)"),
            }),
        },
    }
}

fn env_u64(name: &str, default: u64) -> Result<u64, SmtpError> {
    match std::env::var(name) {
        Err(_) => Ok(default),
        Ok(raw) => raw.parse().map_err(|_| SmtpError::Config {
            message: format!("{name}={raw} is not a number"),
        }),
    }
}

fn env_usize(name: &str, default: usize) -> Result<usize, SmtpError> {
    match std::env::var(name) {
        Err(_) => Ok(default),
        Ok(raw) => raw.parse().map_err(|_| SmtpError::Config {
            message: format!("{name}={raw} is not a number"),
        }),
    }
}

/// Warns when the credentials file is group/world-readable (Unix),
/// since it holds plaintext dev passwords. No-op on Windows.
fn warn_if_world_readable(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path)
            && meta.permissions().mode() & 0o077 != 0
        {
            tracing::warn!(
                path = %path.display(),
                "credentials file is group/world-accessible; restrict to 0600"
            );
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

/// Parses an optional socket-address env var.
fn env_addr(name: &str) -> Result<Option<SocketAddr>, SmtpError> {
    match std::env::var(name) {
        Ok(raw) if !raw.is_empty() => raw.parse().map(Some).map_err(|_| SmtpError::Config {
            message: format!("{name}={raw} is not a host:port address"),
        }),
        _ => Ok(None),
    }
}

/// Loads submission credentials from a `username:password`-per-line
/// file (the dev AUTH bootstrap, replaced by ficina-identity in M9).
/// Blank lines and `#` comments are ignored.
///
/// # Errors
/// [`SmtpError::Config`] when the file cannot be read or a line is
/// malformed (no colon).
pub fn load_credentials(path: &std::path::Path) -> Result<HashMap<String, String>, SmtpError> {
    warn_if_world_readable(path);
    let contents = std::fs::read_to_string(path).map_err(|error| SmtpError::Config {
        message: format!("reading credentials file {}: {error}", path.display()),
    })?;
    let mut map = HashMap::new();
    for (lineno, line) in contents.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (user, pass) = line.split_once(':').ok_or_else(|| SmtpError::Config {
            message: format!(
                "credentials file {} line {}: expected username:password",
                path.display(),
                lineno + 1
            ),
        })?;
        map.insert(user.to_owned(), pass.to_owned());
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn defaults_are_valid_and_rfc_compliant() {
        // Only assert on parseability — env mutation would race other
        // tests, and the numeric floors are compile-time asserts above.
        let addr: SocketAddr = DEFAULT_ADDR.parse().unwrap();
        assert_eq!(addr.port(), 2525);
    }
}
