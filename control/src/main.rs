//! Binary entry point for the Ficina control plane: read config from the
//! environment, wire the store + identity, serve `/control/*`. All logic lives
//! in the library (new-component skill).
//!
//! Environment:
//! - `DATABASE_URL` — the Postgres system of record (required),
//! - `FICINA_BLOB_DIR` — the on-disk blob backend, shared with the other
//!   services (required; the store needs it, though the control plane serves
//!   no blobs),
//! - `FICINA_IDENTITY_ISSUER` — the OIDC issuer URL (required; the control
//!   plane resolves operator tokens through `ficina-identity`),
//! - `FICINA_CONTROL_ADDR` — the internal bind address (default
//!   `0.0.0.0:8090`; TLS is terminated by the front proxy).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use ficina_control::{control_state, serve};
use ficina_identity::{Identity, IdentityConfig};
use ficina_store::{BlobStore, Store};

/// Per-object blob ceiling; matches the other services (the control plane
/// never stores blobs, but the store constructor needs a ceiling).
const BLOB_MAX_BYTES: usize = 50 * 1024 * 1024;
/// Default internal bind (the front proxy terminates TLS and forwards here).
const DEFAULT_ADDR: &str = "0.0.0.0:8090";

#[tokio::main]
async fn main() -> ExitCode {
    let addr = match bind_addr() {
        Ok(addr) => addr,
        Err(error) => {
            eprintln!("ficina-control: {error}");
            return ExitCode::FAILURE;
        }
    };

    // `--healthcheck` TCP-probes the bind address over loopback and exits.
    if std::env::args().nth(1).as_deref() == Some("--healthcheck") {
        return match healthcheck(addr).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("ficina-control: {error}");
                ExitCode::FAILURE
            }
        };
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    match run(addr).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(%error, "fatal");
            ExitCode::FAILURE
        }
    }
}

async fn run(addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    let database_url = require_env("DATABASE_URL")?;
    let blob_dir = PathBuf::from(require_env("FICINA_BLOB_DIR")?);
    let issuer = require_env("FICINA_IDENTITY_ISSUER")?;

    let blobs = BlobStore::local(&blob_dir, BLOB_MAX_BYTES)
        .map_err(|e| format!("cannot open blob directory {}: {e}", blob_dir.display()))?;
    let store = Arc::new(
        Store::connect(&database_url, blobs)
            .await
            .map_err(|_| "cannot connect to the database")?,
    );
    // Migrations are owned by the mail services; the control plane only reads
    // and writes control tables that those migrations create. Do not migrate
    // here — a single migrator avoids concurrent-startup races.

    let identity = Identity::new(Arc::clone(&store), IdentityConfig::new(issuer))
        .map_err(|_| "could not initialise the credential authority")?;

    let state = control_state(store, identity);
    tracing::info!(%addr, "ficina-control (multi-tenant control plane) starting");
    serve(addr, state).await?;
    Ok(())
}

async fn healthcheck(addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    let probe = loopback(addr);
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::net::TcpStream::connect(probe),
    )
    .await
    .map_err(|_| "healthcheck: connection timed out")?
    .map_err(|e| format!("healthcheck: {e}"))?;
    Ok(())
}

fn bind_addr() -> Result<SocketAddr, String> {
    let raw = std::env::var("FICINA_CONTROL_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.to_owned());
    raw.parse()
        .map_err(|e| format!("FICINA_CONTROL_ADDR: invalid socket address {raw:?}: {e}"))
}

fn loopback(bind: SocketAddr) -> SocketAddr {
    if bind.ip().is_unspecified() {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), bind.port())
    } else {
        bind
    }
}

fn require_env(key: &str) -> Result<String, String> {
    match std::env::var(key) {
        Ok(v) if !v.is_empty() => Ok(v),
        _ => Err(format!("{key} is required")),
    }
}
