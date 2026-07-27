//! # ficina-jmap — the JMAP API (RFC 8620 core, RFC 8621 mail)
//!
//! An HTTP service over [`ficina_store`]. **A public contract from
//! merge:** the web app, desktop cache, and compat adapters speak it, so
//! every surface changes additively forever (see
//! `docs/design/jmap-api.md`).
//!
//! Every request reaches mail data only through the store's `for_account`
//! door: the bearer token resolves to `(tenant, user)` and both the
//! tenant and account claims come from the token, never a request body.
//! Because that door is scoped to `(tenant, user)` in the store itself,
//! handlers cannot reach another account's rows and carry no per-call
//! `owns_*` guards. Interim bearer auth is behind [`state::Account`]
//! resolution and swaps for ficina-identity (OIDC) later without touching
//! method code.

pub mod api;
pub mod blob;
pub mod error;
pub mod jtypes;
pub mod push;
pub mod server;
pub mod session;
pub mod state;

pub use push::PushHub;
pub use server::{app, app_state, serve};
pub use state::{AppState, Limits};
