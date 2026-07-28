//! # ficina-store — the tenant-scoped message store
//!
//! Owns: mailboxes, messages, flags, threads, and blob metadata on
//! PostgreSQL, with message bytes in Garage (S3). This is where customer
//! data comes to rest and where **tenancy is structural**: mail data is
//! reachable only through a [`TenantStore`], obtained solely via
//! [`Store::for_tenant`], and every query it issues carries its tenant
//! predicate by construction (see `docs/design/message-store.md`).

pub mod account;
pub mod account_imap;
pub mod account_sieve;
pub mod blob;
pub mod changes;
pub mod error;
pub mod id;
pub mod identity;
pub mod message;
pub mod model;
pub mod store;
pub mod thread;

pub use account::AccountStore;
pub use account_imap::{ImapEntry, ImapMailbox, ImapSearchRow};
pub use account_sieve::{OutboundAction, SieveDelivery, SieveScriptMeta};
pub use blob::BlobStore;
#[cfg(feature = "garage")]
pub use blob::GarageConfig;
pub use changes::Changes;
pub use error::{Result, StoreError};
pub use id::{BlobId, GroupId, MailboxId, MessageId, TenantId, ThreadId, UserId};
pub use identity::{
    AccessTokenRow, AuthCodeOutcome, AuthCodeRow, CredentialRow, OAuthClient, RefreshTokenRow,
    SigningKeyRow, TotpRow,
};
pub use model::{
    Blob, EmailFilter, EmailQuery, MAX_PAGE, Mailbox, Message, MessageSummary, Page, SortDirection,
};
pub use store::{SEEN, Store, TenantStore};
