//! Runtime configuration for the SMTP service, read from environment.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use crate::error::SmtpError;

/// Environment variable naming the socket address to listen on.
pub const ENV_ADDR: &str = "ALO_SMTP_ADDR";
/// Environment variable naming the hostname used in banners/replies.
pub const ENV_HOSTNAME: &str = "ALO_SMTP_HOSTNAME";
/// Environment variable naming the spool directory.
pub const ENV_SPOOL_DIR: &str = "ALO_SMTP_SPOOL_DIR";
/// Environment variable naming the **durable** blob directory for local
/// delivery (message bytes on disk). Defaults to `./blobs`.
pub const ENV_BLOB_DIR: &str = "ALO_SMTP_BLOB_DIR";
/// Environment variable for the maximum message size in octets.
pub const ENV_MAX_MESSAGE_SIZE: &str = "ALO_SMTP_MAX_MESSAGE_SIZE";
/// Environment variable for the per-transaction recipient limit.
pub const ENV_MAX_RCPT: &str = "ALO_SMTP_MAX_RCPT";
/// Environment variable for the concurrent-connection cap.
pub const ENV_MAX_CONNECTIONS: &str = "ALO_SMTP_MAX_CONNECTIONS";
/// Environment flag enabling outbound delivery (off by default — see
/// the relay-safety note on [`OutboundConfig`]).
pub const ENV_OUTBOUND_ENABLED: &str = "ALO_SMTP_OUTBOUND_ENABLED";
/// Environment variable routing all outbound mail to one host.
pub const ENV_SMARTHOST: &str = "ALO_SMTP_SMARTHOST";
/// Environment variable for the retry base delay in seconds.
pub const ENV_RETRY_BASE_SECS: &str = "ALO_SMTP_RETRY_BASE_SECS";
/// Environment variable for the retry cap in seconds.
pub const ENV_RETRY_CAP_SECS: &str = "ALO_SMTP_RETRY_CAP_SECS";
/// Environment variable for the maximum delivery attempts.
pub const ENV_MAX_ATTEMPTS: &str = "ALO_SMTP_MAX_ATTEMPTS";
/// Environment variable for the queue polling interval in seconds.
pub const ENV_QUEUE_INTERVAL_SECS: &str = "ALO_SMTP_QUEUE_INTERVAL_SECS";
/// Environment variable for the submission (STARTTLS) listener address.
pub const ENV_SUBMISSION_ADDR: &str = "ALO_SMTP_SUBMISSION_ADDR";
/// Environment variable for the implicit-TLS submission listener address.
pub const ENV_IMPLICIT_TLS_ADDR: &str = "ALO_SMTP_IMPLICIT_TLS_ADDR";
/// Environment variable for the TRUSTED INTERNAL submission listener address.
///
/// This listener runs the full submission pipeline (RFC 6409 fixups + DKIM +
/// spool) but with NO AUTH — it trusts its caller (the co-located `alo-jmap`,
/// which has already authenticated the user and binds `MAIL FROM` to that
/// user). It MUST be network-isolated: bound inside the container and never
/// published to the host/internet. `None` disables it. See
/// `docs/design/email-submission.md`.
pub const ENV_INTERNAL_SUBMISSION_ADDR: &str = "ALO_SMTP_INTERNAL_SUBMISSION_ADDR";
/// Environment variable for the TLS certificate PEM path.
pub const ENV_TLS_CERT: &str = "ALO_SMTP_TLS_CERT";
/// Environment variable for the TLS private-key PEM path.
pub const ENV_TLS_KEY: &str = "ALO_SMTP_TLS_KEY";
/// Environment variable listing the domains this server hosts
/// (comma-separated). The MX anti-open-relay guard: only these
/// domains' recipients are accepted on port 25.
pub const ENV_LOCAL_DOMAINS: &str = "ALO_SMTP_LOCAL_DOMAINS";
/// Environment flag permitting self-signed certificate generation when
/// no PEM is configured (development only).
pub const ENV_ALLOW_SELF_SIGNED: &str = "ALO_SMTP_ALLOW_SELF_SIGNED";
/// Environment variable for the DKIM signing domain (`d=`).
pub const ENV_DKIM_DOMAIN: &str = "ALO_SMTP_DKIM_DOMAIN";
/// Environment variable for the DKIM selector (`s=`).
pub const ENV_DKIM_SELECTOR: &str = "ALO_SMTP_DKIM_SELECTOR";
/// Environment variable for the DKIM private-key PEM path.
pub const ENV_DKIM_KEY: &str = "ALO_SMTP_DKIM_KEY";
/// Environment variable selecting the DKIM algorithm (`ed25519` or the
/// default `rsa`).
pub const ENV_DKIM_ALGORITHM: &str = "ALO_SMTP_DKIM_ALGORITHM";
/// Environment variable naming the Rspamd controller URL
/// (`http://host:port`); unset disables spam scanning.
pub const ENV_RSPAMD_URL: &str = "ALO_SMTP_RSPAMD_URL";
/// Environment variable for the Rspamd call timeout in seconds.
pub const ENV_RSPAMD_TIMEOUT_SECS: &str = "ALO_SMTP_RSPAMD_TIMEOUT_SECS";
/// Environment variable for the MTA-STS policy listener address; unset
/// disables serving the policy.
pub const ENV_MTA_STS_ADDR: &str = "ALO_SMTP_MTA_STS_ADDR";
/// Environment variable for the MTA-STS mode (`enforce`/`testing`/`none`).
pub const ENV_MTA_STS_MODE: &str = "ALO_SMTP_MTA_STS_MODE";
/// Environment variable for the MTA-STS MX patterns (comma-separated;
/// defaults to the server hostname).
pub const ENV_MTA_STS_MX: &str = "ALO_SMTP_MTA_STS_MX";
/// Environment variable for the MTA-STS `max_age` in seconds.
pub const ENV_MTA_STS_MAX_AGE: &str = "ALO_SMTP_MTA_STS_MAX_AGE";
/// Environment variable for an explicit MTA-STS policy id (derived from
/// the policy content when unset).
pub const ENV_MTA_STS_ID: &str = "ALO_SMTP_MTA_STS_ID";

const DEFAULT_ADDR: &str = "0.0.0.0:2525";
const DEFAULT_HOSTNAME: &str = "alo.test";
const DEFAULT_SPOOL_DIR: &str = "./spool";
const DEFAULT_BLOB_DIR: &str = "./blobs";
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
/// Default Rspamd call timeout (a local scanner should answer quickly;
/// on timeout the message is fail-closed deferred).
const DEFAULT_RSPAMD_TIMEOUT_SECS: u64 = 10;
/// Default MTA-STS `max_age`: one week (RFC 8461 recommends a long TTL
/// once a policy is stable).
const DEFAULT_MTA_STS_MAX_AGE: u32 = 604_800;

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
    /// Root of the durable on-disk blob store for local delivery (message
    /// bytes). Only used when `database_url` is set.
    pub blob_dir: PathBuf,
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
    /// Trusted internal submission listener (no auth, docker-network only);
    /// `None` disables it. Must never be published to the internet.
    pub internal_submission_addr: Option<SocketAddr>,
    /// TLS certificate + key PEM paths. `None` generates a self-signed
    /// certificate at startup (development only).
    pub tls: Option<TlsPaths>,
    /// Hosted domains (lowercased) for the MX anti-open-relay guard.
    /// Empty accepts all recipients (development); a non-empty list is
    /// required before outbound delivery may be enabled.
    pub local_domains: Vec<String>,
    /// Whether a self-signed certificate may be generated when no PEM
    /// is configured. Must be set explicitly so a production server
    /// never silently presents an untrusted cert (opportunistic-TLS
    /// MITM exposure).
    pub allow_self_signed: bool,
    /// DKIM signing for submitted mail; `None` disables signing.
    pub dkim: Option<DkimSigning>,
    /// Rspamd spam-scoring endpoint; `None` disables scanning (mail
    /// flows unscanned). When set, a scanner outage fails closed.
    pub rspamd: Option<RspamdSettings>,
    /// MTA-STS policy endpoint; `None` disables serving the policy.
    pub mta_sts: Option<MtaStsSettings>,
    /// PostgreSQL URL for the message store. When set (and the MX has a
    /// non-empty `local_domains` list), inbound mail for a hosted domain is
    /// delivered into the store (with Sieve at the boundary) instead of the
    /// spool. `None` keeps the receive-only spool behaviour.
    pub database_url: Option<String>,
}

/// Rspamd integration settings (M4b).
#[derive(Debug, Clone)]
pub struct RspamdSettings {
    /// Controller URL (`http://host:port`), kept for logging.
    pub url: String,
    /// The validated client (built once at config load).
    pub client: std::sync::Arc<crate::rspamd::RspamdClient>,
}

/// MTA-STS serving settings (M4b): where to serve the policy and the
/// validated policy itself.
#[derive(Debug, Clone)]
pub struct MtaStsSettings {
    /// Listener address for the (plaintext, proxy-fronted) policy HTTP
    /// endpoint.
    pub addr: SocketAddr,
    /// The validated, pre-rendered policy.
    pub policy: alo_auth_mail::mta_sts::MtaStsPolicy,
}

/// DKIM signing configuration (M4). The key path is always explicit —
/// never defaulted into the repo tree — and permission-checked at load.
#[derive(Debug, Clone)]
pub struct DkimSigning {
    /// Signing domain (`d=`).
    pub domain: String,
    /// Selector (`s=`), addressing the key for rotation.
    pub selector: String,
    /// Path to the PKCS#8 PEM private key.
    pub key_path: PathBuf,
    /// `true` for Ed25519 (RFC 8463), `false` for RSA.
    pub ed25519: bool,
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
        let blob_dir = PathBuf::from(
            std::env::var(ENV_BLOB_DIR).unwrap_or_else(|_| DEFAULT_BLOB_DIR.to_owned()),
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
        let internal_submission_addr = env_addr(ENV_INTERNAL_SUBMISSION_ADDR)?;

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

        let dkim = Self::dkim_from_env()?;
        let rspamd = Self::rspamd_from_env()?;
        let mta_sts = Self::mta_sts_from_env(&hostname)?;
        let database_url = std::env::var("DATABASE_URL").ok().filter(|s| !s.is_empty());
        // Local delivery into the store needs a hosted-domains list, so a
        // recipient can be classified local before it is resolved.
        if database_url.is_some() && local_domains.is_empty() {
            return Err(SmtpError::Config {
                message: format!(
                    "DATABASE_URL is set for local delivery but {ENV_LOCAL_DOMAINS} is empty \
                     (no way to tell which recipients are local)"
                ),
            });
        }

        Ok(Self {
            bind_addr,
            hostname,
            spool_dir,
            blob_dir,
            max_message_size,
            max_rcpt,
            max_connections,
            outbound,
            submission_addr,
            implicit_tls_addr,
            internal_submission_addr,
            tls,
            local_domains,
            allow_self_signed,
            dkim,
            rspamd,
            mta_sts,
            database_url,
        })
    }

    /// Reads the Rspamd endpoint; validates the URL now so a typo fails
    /// at startup, naming the variable.
    fn rspamd_from_env() -> Result<Option<RspamdSettings>, SmtpError> {
        let url = match std::env::var(ENV_RSPAMD_URL) {
            Ok(url) if !url.is_empty() => url,
            _ => return Ok(None),
        };
        let timeout = Duration::from_secs(
            env_u64(ENV_RSPAMD_TIMEOUT_SECS, DEFAULT_RSPAMD_TIMEOUT_SECS)?.max(1),
        );
        let client = crate::rspamd::RspamdClient::from_url(&url, timeout).map_err(|message| {
            SmtpError::Config {
                message: format!("{ENV_RSPAMD_URL}: {message}"),
            }
        })?;
        Ok(Some(RspamdSettings {
            url,
            client: std::sync::Arc::new(client),
        }))
    }

    /// Reads and validates the MTA-STS policy; only served when an
    /// address is configured.
    fn mta_sts_from_env(hostname: &str) -> Result<Option<MtaStsSettings>, SmtpError> {
        use alo_auth_mail::mta_sts::{MtaStsPolicy, StsMode};
        let Some(addr) = env_addr(ENV_MTA_STS_ADDR)? else {
            return Ok(None);
        };
        let mode = match std::env::var(ENV_MTA_STS_MODE) {
            Ok(m) if !m.is_empty() => StsMode::parse(&m).ok_or_else(|| SmtpError::Config {
                message: format!("{ENV_MTA_STS_MODE}={m} must be enforce/testing/none"),
            })?,
            _ => StsMode::Enforce,
        };
        let mx: Vec<String> = std::env::var(ENV_MTA_STS_MX)
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();
        // Default to this server's hostname when no MX patterns are given.
        let mx = if mx.is_empty() {
            vec![hostname.to_owned()]
        } else {
            mx
        };
        let max_age = u32::try_from(env_u64(
            ENV_MTA_STS_MAX_AGE,
            u64::from(DEFAULT_MTA_STS_MAX_AGE),
        )?)
        .map_err(|_| SmtpError::Config {
            message: format!("{ENV_MTA_STS_MAX_AGE} is out of range"),
        })?;
        let id = std::env::var(ENV_MTA_STS_ID).ok().filter(|s| !s.is_empty());
        let policy =
            MtaStsPolicy::new(mode, mx, max_age, id).map_err(|error| SmtpError::Config {
                message: format!("MTA-STS policy invalid: {error}"),
            })?;
        Ok(Some(MtaStsSettings { addr, policy }))
    }

    /// Reads DKIM signing config; all three of domain/selector/key must
    /// be set together, or none (signing disabled).
    fn dkim_from_env() -> Result<Option<DkimSigning>, SmtpError> {
        let domain = std::env::var(ENV_DKIM_DOMAIN)
            .ok()
            .filter(|s| !s.is_empty());
        let selector = std::env::var(ENV_DKIM_SELECTOR)
            .ok()
            .filter(|s| !s.is_empty());
        let key = std::env::var(ENV_DKIM_KEY).ok().filter(|s| !s.is_empty());
        match (domain, selector, key) {
            (Some(domain), Some(selector), Some(key)) => {
                let ed25519 = std::env::var(ENV_DKIM_ALGORITHM)
                    .map(|a| a.eq_ignore_ascii_case("ed25519"))
                    .unwrap_or(false);
                Ok(Some(DkimSigning {
                    domain,
                    selector,
                    key_path: PathBuf::from(key),
                    ed25519,
                }))
            }
            (None, None, None) => Ok(None),
            _ => Err(SmtpError::Config {
                message: format!(
                    "{ENV_DKIM_DOMAIN}, {ENV_DKIM_SELECTOR}, and {ENV_DKIM_KEY} must be set together"
                ),
            }),
        }
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

/// Parses an optional socket-address env var.
fn env_addr(name: &str) -> Result<Option<SocketAddr>, SmtpError> {
    match std::env::var(name) {
        Ok(raw) if !raw.is_empty() => raw.parse().map(Some).map_err(|_| SmtpError::Config {
            message: format!("{name}={raw} is not a host:port address"),
        }),
        _ => Ok(None),
    }
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
