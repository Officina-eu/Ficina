//! # ficina-store — the tenant-scoped message store
//!
//! Owns: mailboxes, messages, flags, threads, and blob metadata on
//! PostgreSQL, with message bytes in Garage (S3). This is where customer
//! data comes to rest and where **isolation is structural**: tenant-level
//! administration goes through a [`TenantStore`] ([`Store::for_tenant`]),
//! while all user-owned mail data is reachable only through an
//! [`AccountStore`], obtained solely via [`Store::for_account`]. Each
//! query an `AccountStore` issues carries `tenant_id = $tenant AND
//! user_id = $user` by construction, so neither a cross-tenant nor a
//! cross-account access is representable in the API (see
//! `docs/design/message-store.md`).

pub mod account;
pub mod auth;
pub mod blob;
pub mod changes;
pub mod error;
pub mod id;
pub mod message;
pub mod model;
pub mod store;
pub mod thread;

pub use account::AccountStore;
pub use auth::IssuedToken;
pub use blob::BlobStore;
#[cfg(feature = "garage")]
pub use blob::GarageConfig;
pub use changes::Changes;
pub use error::{Result, StoreError};
pub use id::{BlobId, MailboxId, MessageId, TenantId, ThreadId, UserId};
pub use model::{
    Blob, EmailFilter, EmailQuery, MAX_PAGE, Mailbox, Message, MessageSummary, Page, SortDirection,
};
pub use store::{SEEN, Store, TenantStore};
