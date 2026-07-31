//! Destination resolution for outbound delivery, RFC 5321 §5.1.
//!
//! Rules implemented: MX records sorted by preference; when no MX
//! exists but the domain does, the implicit MX is the domain itself;
//! a null MX (single MX with preference 0 and target ".", RFC 7505)
//! means the domain never accepts mail — permanent failure; NXDOMAIN
//! is permanent; DNS timeouts/server failures are transient.
//!
//! The trait exists so the queue can be tested without the network;
//! [`DnsResolver`] is the production implementation (hickory).

use std::net::IpAddr;

use hickory_resolver::TokioResolver;
use hickory_resolver::net::NetError;
use hickory_resolver::proto::rr::RData;

/// Why resolution failed, split by retry semantics.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResolveFailure {
    /// The domain does not exist or explicitly refuses mail
    /// (NXDOMAIN, or RFC 7505 null MX) — generate a DSN, never retry.
    #[error("permanent resolution failure: {reason}")]
    Permanent {
        /// Human-readable cause for the DSN diagnostic.
        reason: String,
    },
    /// DNS infrastructure trouble (timeout, SERVFAIL) — retry later.
    #[error("transient resolution failure: {reason}")]
    Transient {
        /// Human-readable cause for logs/state.
        reason: String,
    },
}

/// An MX target ready to try, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailHost {
    /// Hostname to connect to (display/logging; connection uses `ips`).
    pub host: String,
    /// Resolved addresses for the host, in try order.
    pub ips: Vec<IpAddr>,
}

/// Boxed future alias for the object-safe async trait method below
/// (hand-desugared so we don't pull the async-trait crate for one
/// trait).
pub type ResolveFuture<'a> = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<Vec<MailHost>, ResolveFailure>> + Send + 'a>,
>;

/// Resolves a mail domain to an ordered list of hosts to attempt.
pub trait MxResolve: Send + Sync {
    /// Ordered delivery targets for `domain` per RFC 5321 §5.1.
    ///
    /// # Errors
    /// [`ResolveFailure`] with retry semantics encoded.
    fn resolve<'a>(&'a self, domain: &'a str) -> ResolveFuture<'a>;
}

/// Production resolver over the system's configured DNS.
pub struct DnsResolver {
    inner: TokioResolver,
}

impl DnsResolver {
    /// Builds a resolver from the system configuration.
    ///
    /// # Errors
    /// Returns the underlying error when the system DNS configuration
    /// cannot be read.
    pub fn from_system() -> Result<Self, NetError> {
        let inner = TokioResolver::builder_tokio()?.build()?;
        Ok(Self { inner })
    }

    async fn lookup_ips(&self, host: &str) -> Result<Vec<IpAddr>, ResolveFailure> {
        match self.inner.lookup_ip(host).await {
            Ok(lookup) => Ok(lookup.iter().collect()),
            Err(error) => Err(classify(&error, host)),
        }
    }
}

impl MxResolve for DnsResolver {
    fn resolve<'a>(&'a self, domain: &'a str) -> ResolveFuture<'a> {
        Box::pin(async move {
            match self.inner.mx_lookup(domain).await {
                Ok(lookup) => {
                    // Extract MX rdata from the answer records (§5.1).
                    let mut records: Vec<(u16, String)> = lookup
                        .answers()
                        .iter()
                        .filter_map(|record| match &record.data {
                            RData::MX(mx) => Some((mx.preference, mx.exchange.to_utf8())),
                            _ => None,
                        })
                        .collect();
                    if let [(preference, exchange)] = records.as_slice()
                        && *preference == 0
                        && (exchange == "." || exchange.is_empty())
                    {
                        // RFC 7505 null MX: the domain refuses mail.
                        return Err(ResolveFailure::Permanent {
                            reason: format!("domain {domain} declares a null MX (RFC 7505)"),
                        });
                    }
                    if records.is_empty() {
                        return Err(ResolveFailure::Transient {
                            reason: format!("{domain}: empty MX answer"),
                        });
                    }
                    // §5.1: sort by preference, lower first.
                    records.sort_by_key(|(preference, _)| *preference);
                    let mut hosts = Vec::new();
                    for (_preference, exchange) in records {
                        let name = exchange.trim_end_matches('.').to_owned();
                        match self.lookup_ips(&name).await {
                            Ok(ips) if !ips.is_empty() => hosts.push(MailHost { host: name, ips }),
                            // An MX whose target has no address is
                            // skipped; others may still work (§5.1).
                            Ok(_) | Err(_) => {
                                tracing::debug!(mx = %name, "MX target did not resolve; skipping");
                            }
                        }
                    }
                    if hosts.is_empty() {
                        return Err(ResolveFailure::Transient {
                            reason: format!("no MX target for {domain} currently resolves"),
                        });
                    }
                    Ok(hosts)
                }
                Err(error) if is_no_records(&error) => {
                    // Implicit MX (§5.1): no MX RRset, use the domain
                    // itself if it has an address.
                    let ips = self.lookup_ips(domain).await?;
                    if ips.is_empty() {
                        return Err(ResolveFailure::Permanent {
                            reason: format!("{domain} has neither MX nor address records"),
                        });
                    }
                    Ok(vec![MailHost {
                        host: domain.to_owned(),
                        ips,
                    }])
                }
                Err(error) => Err(classify(&error, domain)),
            }
        })
    }
}

/// Maps a hickory error to retry semantics: NXDOMAIN/no-records are
/// permanent, everything else (timeouts, SERVFAIL, I/O) transient.
fn classify(error: &NetError, subject: &str) -> ResolveFailure {
    if error.is_nx_domain() {
        ResolveFailure::Permanent {
            reason: format!("{subject}: domain does not exist (NXDOMAIN)"),
        }
    } else if error.is_no_records_found() {
        ResolveFailure::Permanent {
            reason: format!("{subject}: no address records"),
        }
    } else {
        ResolveFailure::Transient {
            reason: format!("{subject}: DNS lookup failed: {error}"),
        }
    }
}

fn is_no_records(error: &NetError) -> bool {
    error.is_no_records_found()
}
