//! Entity types returned across the store's public API.

use time::OffsetDateTime;

use crate::id::{BlobId, MailboxId, MessageId, ThreadId};

/// A stored blob's metadata (for JMAP download).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blob {
    /// Opaque id (the store's — JMAP has no second id space).
    pub id: BlobId,
    /// Size in octets.
    pub size: i64,
    /// Declared Content-Type (served verbatim on download).
    pub content_type: Option<String>,
}

/// Sort direction for `Email/query` (by `receivedAt`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    /// Oldest first.
    Asc,
    /// Newest first (the JMAP default).
    Desc,
}

/// `Email/query` filter conditions — all present ones are ANDed.
#[derive(Debug, Clone, Default)]
pub struct EmailFilter {
    /// `inMailbox`: only emails in this mailbox.
    pub in_mailbox: Option<MailboxId>,
    /// `from` substring match.
    pub from: Option<String>,
    /// `to` substring match.
    pub to: Option<String>,
    /// `subject` substring match.
    pub subject: Option<String>,
    /// `text` full-text match over subject/addresses/body.
    pub text: Option<String>,
    /// `before`: `receivedAt` strictly before.
    pub before: Option<OffsetDateTime>,
    /// `after`: `receivedAt` at or after.
    pub after: Option<OffsetDateTime>,
    /// `hasKeyword`: has this keyword.
    pub has_keyword: Option<String>,
    /// `notKeyword`: lacks this keyword.
    pub not_keyword: Option<String>,
}

/// A full `Email/query` request.
#[derive(Debug, Clone)]
pub struct EmailQuery {
    /// Filter conditions.
    pub filter: EmailFilter,
    /// Sort by `receivedAt` in this direction.
    pub sort: SortDirection,
    /// Bounded window.
    pub page: Page,
}

/// A mailbox with its live counters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mailbox {
    /// Opaque id.
    pub id: MailboxId,
    /// Parent mailbox, or `None` at the root.
    pub parent_id: Option<MailboxId>,
    /// Display name (unique among siblings).
    pub name: String,
    /// JMAP role (`inbox`/`sent`/…), or `None`.
    pub role: Option<String>,
    /// Total messages in the mailbox.
    pub total_messages: i64,
    /// Messages without the `$seen` keyword.
    pub unread_messages: i64,
}

/// A compact message row for mailbox listings (no body).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageSummary {
    /// Opaque id.
    pub id: MessageId,
    /// The thread this message belongs to.
    pub thread_id: ThreadId,
    /// Unfolded subject.
    pub subject: String,
    /// Unfolded `From`.
    pub from_addr: String,
    /// `Date` header, when present.
    pub sent_at: Option<OffsetDateTime>,
    /// When the store received it.
    pub received_at: OffsetDateTime,
    /// Size of the raw message in octets.
    pub size: i64,
}

/// A message's full metadata (the bytes are fetched separately as a blob).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Opaque id.
    pub id: MessageId,
    /// The thread this message belongs to.
    pub thread_id: ThreadId,
    /// The content-addressed blob holding the raw bytes.
    pub blob_id: BlobId,
    /// `Message-ID` header (angle brackets included).
    pub message_id_hdr: Option<String>,
    /// Unfolded subject.
    pub subject: String,
    /// Unfolded `From`.
    pub from_addr: String,
    /// Unfolded `To`.
    pub to_addrs: String,
    /// `Date` header, when present.
    pub sent_at: Option<OffsetDateTime>,
    /// When the store received it.
    pub received_at: OffsetDateTime,
    /// Size of the raw message in octets.
    pub size: i64,
    /// Parsed Authentication-Results SPF result (RFC 8601).
    pub auth_spf: Option<String>,
    /// Parsed Authentication-Results DKIM result.
    pub auth_dkim: Option<String>,
    /// Parsed Authentication-Results DMARC result.
    pub auth_dmarc: Option<String>,
}

/// A bounded page request — every list API takes one, so no call can
/// return an unbounded result set.
#[derive(Debug, Clone, Copy)]
pub struct Page {
    limit: i64,
    offset: i64,
}

/// The largest page any single query will return.
pub const MAX_PAGE: i64 = 500;
/// The largest offset any single query will skip. A deep `OFFSET` makes
/// Postgres scan-and-discard O(offset) rows; bound it (keyset pagination
/// replaces offset paging for large collections in a later pass).
pub const MAX_OFFSET: i64 = 100_000;

impl Page {
    /// A page with `limit` clamped to `1..=MAX_PAGE` and `offset` clamped
    /// to `0..=MAX_OFFSET`.
    pub fn new(limit: i64, offset: i64) -> Self {
        Self {
            limit: limit.clamp(1, MAX_PAGE),
            offset: offset.clamp(0, MAX_OFFSET),
        }
    }

    /// The first page of `limit` rows.
    pub fn first(limit: i64) -> Self {
        Self::new(limit, 0)
    }

    /// The clamped row limit.
    pub fn limit(&self) -> i64 {
        self.limit
    }

    /// The clamped row offset.
    pub fn offset(&self) -> i64 {
        self.offset
    }
}

impl Default for Page {
    fn default() -> Self {
        Self::first(50)
    }
}
