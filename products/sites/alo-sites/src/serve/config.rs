//! Environment configuration of the public serving binary.
//!
//! - `DATABASE_URL` — the Postgres system of record (required; read-only use).
//! - `SITES_DOMAIN` — the apex the service resolves subdomain hosts under,
//!   e.g. `alosites.example` makes `acme.alosites.example` serve the site
//!   with subdomain `acme` (required; the name is the contract used across
//!   `docs/design/sites.md`).
//! - `ALO_SITES_ADDR` — internal bind address (default `0.0.0.0:8081`; TLS
//!   is terminated by the front proxy).

use std::net::SocketAddr;

use thiserror::Error;

/// Default internal bind (the front proxy terminates TLS and forwards here).
pub const DEFAULT_ADDR: &str = "0.0.0.0:8081";

/// Why configuration could not be read — printable to an operator as-is.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// A required variable is absent or empty.
    #[error("{0} is required")]
    Missing(&'static str),
    /// A variable is present but unusable.
    #[error("{name} is invalid: {reason}")]
    Invalid {
        /// The environment variable.
        name: &'static str,
        /// What is wrong with it.
        reason: String,
    },
}

/// Everything the service needs from the environment.
#[derive(Debug, Clone)]
pub struct ServeConfig {
    /// Postgres connection string.
    pub database_url: String,
    /// The apex domain published sites are served under (lowercased).
    pub sites_domain: String,
    /// The bind address.
    pub addr: SocketAddr,
}

impl ServeConfig {
    /// Reads and validates the configuration from the process environment.
    ///
    /// # Errors
    /// [`ConfigError`] naming the offending variable.
    pub fn from_env() -> Result<Self, ConfigError> {
        let database_url = require("DATABASE_URL")?;
        let sites_domain = require("SITES_DOMAIN")?.to_ascii_lowercase();
        if !sites_domain
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
        {
            return Err(ConfigError::Invalid {
                name: "SITES_DOMAIN",
                reason: "must be a bare DNS name (letters, digits, dots, hyphens)".to_owned(),
            });
        }
        let addr = std::env::var("ALO_SITES_ADDR")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| DEFAULT_ADDR.to_owned());
        let addr: SocketAddr = addr.parse().map_err(|_| ConfigError::Invalid {
            name: "ALO_SITES_ADDR",
            reason: format!("`{addr}` is not a host:port address"),
        })?;
        Ok(Self {
            database_url,
            sites_domain,
            addr,
        })
    }
}

fn require(name: &'static str) -> Result<String, ConfigError> {
    std::env::var(name)
        .ok()
        .filter(|v| !v.is_empty())
        .ok_or(ConfigError::Missing(name))
}
