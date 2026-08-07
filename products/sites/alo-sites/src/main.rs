//! Thin binary entry point of the public alo-sites service: read config
//! from the environment, open the read-only public store door, serve. All
//! logic lives in the library (`alo_sites::serve`). This process is the
//! anonymous, internet-facing side of alo Sites — it runs no migrations
//! (the authenticated `alo-jmap` service owns the schema) and can be
//! stopped at any time without touching tenant data.
//!
//! Environment: see [`alo_sites::serve::config`].

use std::net::SocketAddr;
use std::process::ExitCode;

use alo_sites::serve::config::BLOB_MAX_BYTES;
use alo_sites::serve::{AppState, ServeConfig, app};
use alo_store::{BlobStore, SitePublicStore};

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(%error, "fatal");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = ServeConfig::from_env()?;
    let blobs = BlobStore::local(&config.blob_dir, BLOB_MAX_BYTES)
        .map_err(|_| "cannot open the blob directory")?;
    let store = SitePublicStore::connect(&config.database_url, blobs)
        .await
        .map_err(|_| "cannot connect to the database")?;
    let state = AppState::new(store, config.sites_domain.clone());
    let listener = tokio::net::TcpListener::bind(config.addr).await?;
    tracing::info!(addr = %config.addr, sites_domain = %config.sites_domain, "alo-sites listening");
    // ConnectInfo gives the form rate limiter a per-peer fallback key when
    // the service is reached without a proxy's X-Forwarded-For in front.
    axum::serve(
        listener,
        app(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}
