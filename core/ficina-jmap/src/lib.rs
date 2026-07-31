//! # ficina-jmap — the JMAP API (RFC 8620 core, RFC 8621 mail)
//!
//! An HTTP service over [`ficina_store`]. **A public contract from
//! merge:** the web app, desktop cache, and compat adapters speak it, so
//! every surface changes additively forever (see
//! `docs/design/jmap-api.md`).
//!
//! Every request reaches data only through the store's `for_account`
//! door: the bearer token resolves to `(tenant, account)` via
//! [`ficina_identity`] and the tenant claim is never read from a request
//! body. The OpenID Connect / OAuth 2.0 provider is mounted alongside
//! (see [`server::app`]), so one HTTP service serves both JMAP and the
//! IdP.

pub mod admin;
pub mod ai;
pub mod api;
pub mod blob;
pub mod contacts;
pub mod docs;
pub mod error;
pub mod jtypes;
pub mod mime;
pub mod mime_read;
pub mod push;
pub mod schedule;
pub mod security;
pub mod server;
pub mod session;
pub mod settings;
pub mod snooze;
pub mod sieve;
pub mod state;
pub mod submission;

pub use push::PushHub;
pub use server::{app, app_state, serve};
pub use state::{AppState, Limits};
