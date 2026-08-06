//! Billing invoices — the document itself (alo Billing, ADR 0035, wave B1),
//! reached through the account door like [`crate::billing_customers`].
//!
//! An invoice is not a row that gets edited forever: it is a **draft** until
//! it is issued, and issuing (B1.08) draws the next number from the tenant's
//! gapless sequence, stamps the dates and freezes the content. This module
//! owns the draft — creating it, replacing its header, replacing its line set
//! — and reading a document back with its totals. The status column already
//! carries the full lifecycle so nothing about the shape of the table changes
//! when issuing arrives; the only status this module can write is `draft`.
//!
//! **Nothing here stores money it computed.** Net, VAT and gross are derived
//! from the lines on every read by [`crate::billing_totals`], so a total can
//! never drift from the lines that justify it, and no client can influence
//! what a document is worth by sending a number.
//!
//! Lines are written as a **whole set**, in the caller's order, inside one
//! transaction — a draft editor sends the document it wants, not a patch
//! stream, so there is no window in which a document is half-edited and no
//! ambiguity about line order.
//!
//! Tenancy is structural: every statement carries `tenant_id` from the handle,
//! the customer link is re-checked under the same handle before it is written
//! (a guessed id from another tenant is a [`StoreError::NotFound`], never a
//! cross-tenant link), and the database backs that with a composite foreign
//! key on `(tenant_id, customer_id)`.

use std::collections::HashMap;

use time::{Date, OffsetDateTime};

use crate::account::AccountStore;
use crate::billing_field::{bounded, currency, payment_terms_days};
use crate::billing_line::{Line, NewLine, NormalizedLine, normalize_lines};
use crate::billing_totals::{LineFigures, Totals, totals};
use crate::error::{Result, StoreError};
use crate::id::{BillingCustomerId, BillingInvoiceId, BillingLineId};

/// The customer's own reference (a PO number, a cost centre) printed on the
/// document.
pub const INVOICE_REFERENCE_MAX_CHARS: usize = 120;
/// A free-text note printed under the lines (delivery terms, a thank-you, the
/// reverse-charge sentence).
pub const INVOICE_NOTE_MAX_CHARS: usize = 2_000;

/// The columns every read of an invoice selects, in `InvoiceRow` order.
const INVOICE_COLS: &str = "id, customer_id, status, currency, number, issue_date, due_date, \
     payment_terms_days, is_credit_note, credits_invoice_id, reference, note, created_by, \
     created_at, updated_at";

/// The columns every read of a line selects, in `LineRow` order.
const LINE_COLS: &str = "id, line_order, description, unit, qty_milli, unit_price_cents, \
     vat_rate_bp";

/// Where a document is in its life.
///
/// The transitions are `draft → issued → paid` and `issued → void`; a draft is
/// deleted rather than voided, because it never consumed a number. Only a
/// draft is editable — the guard on that lands with the draft lifecycle
/// (B1.07), and issuing with B1.08.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InvoiceStatus {
    /// Editable, unnumbered, not yet a legal document.
    Draft,
    /// Numbered, dated and frozen; owed by the customer.
    Issued,
    /// Settled in full by recorded payments (B1.19).
    Paid,
    /// Issued and then cancelled. The number is kept — the sequence stays
    /// gapless — and the document remains readable.
    Void,
}

impl InvoiceStatus {
    /// The value stored in the `status` column.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Issued => "issued",
            Self::Paid => "paid",
            Self::Void => "void",
        }
    }

    /// Parses a stored status, or `None` if it is not one we know.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "draft" => Some(Self::Draft),
            "issued" => Some(Self::Issued),
            "paid" => Some(Self::Paid),
            "void" => Some(Self::Void),
            _ => None,
        }
    }

    /// Whether the document is still editable.
    pub fn is_draft(self) -> bool {
        matches!(self, Self::Draft)
    }
}

/// The writable header of an invoice, used for both create and update (an
/// update is a full replace — the route layer merges a partial `PATCH` onto
/// the stored record before calling). Lines are written separately, as a set.
///
/// `currency` and `payment_terms_days` are `None` to mean *take the
/// customer's*, which is what a UI that has not asked the user should send.
/// Whatever is resolved is then **stored on the document**: changing a
/// customer's terms next year must not silently restate a document raised
/// this year.
#[derive(Debug, Clone)]
pub struct NewInvoice {
    /// The party billed. Must be one of this tenant's customers.
    pub customer_id: BillingCustomerId,
    /// ISO 4217 code, or `None` for the customer's default.
    pub currency: Option<String>,
    /// Days from issue to due, or `None` for the customer's terms.
    pub payment_terms_days: Option<i32>,
    /// The customer's own reference (PO number), printed on the document.
    pub reference: String,
    /// Free-text note printed under the lines.
    pub note: String,
}

impl NewInvoice {
    /// The blank header a new draft starts from: this customer, their
    /// currency and their terms, no reference and no note. There is
    /// deliberately no [`Default`] — an invoice without a customer is not a
    /// document, and a defaulted (empty) customer id would only fail later,
    /// further from the mistake.
    pub fn for_customer(customer_id: BillingCustomerId) -> Self {
        Self {
            customer_id,
            currency: None,
            payment_terms_days: None,
            reference: String::new(),
            note: String::new(),
        }
    }
}

/// The header of a stored invoice. Its money lives in [`Totals`], computed
/// from the lines.
#[derive(Debug, Clone)]
pub struct Invoice {
    /// Opaque id, unique within the tenant.
    pub id: BillingInvoiceId,
    /// The party billed.
    pub customer_id: BillingCustomerId,
    /// Where the document is in its life.
    pub status: InvoiceStatus,
    /// ISO 4217 code the document was raised in.
    pub currency: String,
    /// The legal document number, `None` while draft.
    pub number: Option<String>,
    /// Date of issue, `None` while draft.
    pub issue_date: Option<Date>,
    /// Date payment is due, `None` while draft.
    pub due_date: Option<Date>,
    /// Payment terms snapshotted from the customer, in days.
    pub payment_terms_days: i32,
    /// Whether this document credits another (B1.09).
    pub is_credit_note: bool,
    /// The document credited, when this is a credit note.
    pub credits_invoice_id: Option<BillingInvoiceId>,
    /// The customer's own reference.
    pub reference: String,
    /// Free-text note.
    pub note: String,
    /// The user who created the document.
    pub created_by: String,
    /// Creation time.
    pub created_at: OffsetDateTime,
    /// Last modification time — moved by a header edit and by a line edit,
    /// since both change what the document says.
    pub updated_at: OffsetDateTime,
}

/// An invoice as a list entry: the header and what it is worth, without the
/// lines. The totals are computed, never read from a column.
#[derive(Debug, Clone)]
pub struct InvoiceSummary {
    /// The header.
    pub invoice: Invoice,
    /// Net, VAT breakdown and gross, derived from the lines.
    pub totals: Totals,
}

/// A whole document: header, lines in print order, and the totals derived
/// from those lines.
#[derive(Debug, Clone)]
pub struct InvoiceDocument {
    /// The header.
    pub invoice: Invoice,
    /// The lines, in print order.
    pub lines: Vec<Line>,
    /// Net, VAT breakdown and gross, derived from `lines`.
    pub totals: Totals,
}

/// The header, validated and with the customer's defaults resolved.
#[derive(Debug)]
struct NormalizedInvoice {
    customer_id: String,
    currency: String,
    payment_terms_days: i32,
    reference: String,
    note: String,
}

impl AccountStore {
    /// Resolves a header against **this tenant's** customer: the customer must
    /// exist under this handle, so a guessed id from another tenant is a
    /// `NotFound`, and it must be active — archiving a customer means "we no
    /// longer bill them", so raising a new document for one is a mistake
    /// worth reporting rather than obeying.
    async fn normalize_invoice(&self, input: &NewInvoice) -> Result<NormalizedInvoice> {
        let customer = self
            .billing_customer(&input.customer_id)
            .await?
            .ok_or(StoreError::NotFound)?;
        if customer.is_archived() {
            return Err(StoreError::Validation(
                "the customer is archived; restore it before billing it again".to_owned(),
            ));
        }
        let resolved_currency = match input.currency.as_deref() {
            Some(code) => currency(code)?,
            None => customer.currency,
        };
        let resolved_terms = match input.payment_terms_days {
            Some(days) => payment_terms_days(days)?,
            None => customer.payment_terms_days,
        };
        Ok(NormalizedInvoice {
            customer_id: customer.id.as_str().to_owned(),
            currency: resolved_currency,
            payment_terms_days: resolved_terms,
            reference: bounded("reference", &input.reference, INVOICE_REFERENCE_MAX_CHARS)?,
            note: bounded("note", &input.note, INVOICE_NOTE_MAX_CHARS)?,
        })
    }

    /// Creates a **draft** invoice with no lines — the state a new document
    /// starts in. It carries no number and no dates by construction; only
    /// issuing (B1.08) assigns those.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] when the customer is not this tenant's;
    /// [`StoreError::Validation`] when the customer is archived or a header
    /// field breaks its rule; [`StoreError::Db`] on failure.
    pub async fn create_billing_invoice(&self, input: &NewInvoice) -> Result<BillingInvoiceId> {
        let header = self.normalize_invoice(input).await?;
        let id = BillingInvoiceId::generate();
        sqlx::query(
            "INSERT INTO billing_invoices (tenant_id, id, customer_id, status, currency, \
                 payment_terms_days, reference, note, created_by) \
             VALUES ($1, $2, $3, 'draft', $4, $5, $6, $7, $8)",
        )
        .bind(self.tenant.as_str())
        .bind(id.as_str())
        .bind(&header.customer_id)
        .bind(&header.currency)
        .bind(header.payment_terms_days)
        .bind(&header.reference)
        .bind(&header.note)
        .bind(self.user.as_str())
        .execute(&self.pool)
        .await
        .map_err(StoreError::Db)?;
        Ok(id)
    }

    /// The tenant's invoices, newest first, each with its computed totals.
    /// `status` filters; `None` lists everything.
    ///
    /// The lines of every listed document are fetched in one further
    /// statement, not one per document.
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn billing_invoices(
        &self,
        status: Option<InvoiceStatus>,
    ) -> Result<Vec<InvoiceSummary>> {
        let status = status.map(InvoiceStatus::as_str);
        let rows = sqlx::query_as::<_, InvoiceRow>(&format!(
            "SELECT {INVOICE_COLS} FROM billing_invoices \
             WHERE tenant_id = $1 AND ($2::text IS NULL OR status = $2) \
             ORDER BY created_at DESC, id"
        ))
        .bind(self.tenant.as_str())
        .bind(status)
        .fetch_all(&self.pool)
        .await
        .map_err(StoreError::Db)?;

        let figures = sqlx::query_as::<_, FiguresRow>(
            "SELECT invoice_id, qty_milli, unit_price_cents, vat_rate_bp \
             FROM billing_invoice_lines \
             WHERE tenant_id = $1 AND invoice_id IN ( \
                 SELECT id FROM billing_invoices \
                 WHERE tenant_id = $1 AND ($2::text IS NULL OR status = $2))",
        )
        .bind(self.tenant.as_str())
        .bind(status)
        .fetch_all(&self.pool)
        .await
        .map_err(StoreError::Db)?;

        let mut by_invoice: HashMap<String, Vec<LineFigures>> = HashMap::new();
        for row in figures {
            by_invoice
                .entry(row.invoice_id)
                .or_default()
                .push(LineFigures {
                    qty_milli: row.qty_milli,
                    unit_price_cents: row.unit_price_cents,
                    vat_rate_bp: row.vat_rate_bp,
                });
        }

        rows.into_iter()
            .map(|row| {
                let lines = by_invoice.remove(&row.id).unwrap_or_default();
                Ok(InvoiceSummary {
                    invoice: row.into_invoice()?,
                    totals: totals(&lines),
                })
            })
            .collect()
    }

    /// One document of the tenant with its lines and totals, or `None` —
    /// including when the id belongs to another tenant (indistinguishable by
    /// design).
    ///
    /// # Errors
    /// [`StoreError::Db`] on failure.
    pub async fn billing_invoice(&self, id: &BillingInvoiceId) -> Result<Option<InvoiceDocument>> {
        let Some(row) = sqlx::query_as::<_, InvoiceRow>(&format!(
            "SELECT {INVOICE_COLS} FROM billing_invoices WHERE tenant_id = $1 AND id = $2"
        ))
        .bind(self.tenant.as_str())
        .bind(id.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(StoreError::Db)?
        else {
            return Ok(None);
        };
        let lines: Vec<Line> = sqlx::query_as::<_, LineRow>(&format!(
            "SELECT {LINE_COLS} FROM billing_invoice_lines \
             WHERE tenant_id = $1 AND invoice_id = $2 ORDER BY line_order"
        ))
        .bind(self.tenant.as_str())
        .bind(id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(StoreError::Db)?
        .into_iter()
        .map(LineRow::into_line)
        .collect();
        let figures: Vec<LineFigures> = lines.iter().map(Line::figures).collect();
        Ok(Some(InvoiceDocument {
            invoice: row.into_invoice()?,
            lines,
            totals: totals(&figures),
        }))
    }

    /// Replaces the writable header of an invoice: customer, currency, terms,
    /// reference and note. Status, number and dates are not writable here —
    /// they move only through the lifecycle actions.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] when the invoice or the customer is not this
    /// tenant's; [`StoreError::Validation`] as for create; [`StoreError::Db`]
    /// on failure.
    pub async fn update_billing_invoice(
        &self,
        id: &BillingInvoiceId,
        input: &NewInvoice,
    ) -> Result<()> {
        let header = self.normalize_invoice(input).await?;
        let done = sqlx::query(
            "UPDATE billing_invoices SET customer_id = $3, currency = $4, \
                 payment_terms_days = $5, reference = $6, note = $7, updated_at = now() \
             WHERE tenant_id = $1 AND id = $2",
        )
        .bind(self.tenant.as_str())
        .bind(id.as_str())
        .bind(&header.customer_id)
        .bind(&header.currency)
        .bind(header.payment_terms_days)
        .bind(&header.reference)
        .bind(&header.note)
        .execute(&self.pool)
        .await
        .map_err(StoreError::Db)?;
        if done.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// Replaces the whole line set of an invoice, in the caller's order, in
    /// one transaction: either the document reads exactly as the caller sent
    /// it or it is untouched. Line positions are assigned 0-based from that
    /// order, so what was sent is what prints.
    ///
    /// Every line is validated **before** anything is written, so a document
    /// is never left half-replaced by a bad line at the end.
    ///
    /// # Errors
    /// [`StoreError::NotFound`] when the invoice is not this tenant's;
    /// [`StoreError::Validation`] when the set is too long or a line breaks a
    /// field rule (the message names the line's position);
    /// [`StoreError::Db`] on failure.
    pub async fn set_billing_invoice_lines(
        &self,
        id: &BillingInvoiceId,
        lines: &[NewLine],
    ) -> Result<()> {
        let lines: Vec<NormalizedLine> = normalize_lines(lines)?;

        let mut tx = self.pool.begin().await.map_err(StoreError::Db)?;
        // Lock the document for the whole replacement: two editors saving at
        // once serialise here instead of interleaving their line sets. The
        // draft-only guard lands on this same lock in B1.07.
        let found: Option<String> = sqlx::query_scalar(
            "SELECT id FROM billing_invoices WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
        )
        .bind(self.tenant.as_str())
        .bind(id.as_str())
        .fetch_optional(&mut *tx)
        .await
        .map_err(StoreError::Db)?;
        if found.is_none() {
            // Dropping the transaction rolls it back; nothing was written.
            return Err(StoreError::NotFound);
        }

        sqlx::query("DELETE FROM billing_invoice_lines WHERE tenant_id = $1 AND invoice_id = $2")
            .bind(self.tenant.as_str())
            .bind(id.as_str())
            .execute(&mut *tx)
            .await
            .map_err(StoreError::Db)?;

        for (index, line) in lines.iter().enumerate() {
            let order = i32::try_from(index)
                .map_err(|_| StoreError::Validation("a document has too many lines".to_owned()))?;
            sqlx::query(
                "INSERT INTO billing_invoice_lines (tenant_id, invoice_id, id, line_order, \
                     description, unit, qty_milli, unit_price_cents, vat_rate_bp) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            )
            .bind(self.tenant.as_str())
            .bind(id.as_str())
            .bind(BillingLineId::generate().as_str())
            .bind(order)
            .bind(&line.description)
            .bind(&line.unit)
            .bind(line.qty_milli)
            .bind(line.unit_price_cents)
            .bind(line.vat_rate_bp)
            .execute(&mut *tx)
            .await
            .map_err(StoreError::Db)?;
        }

        sqlx::query(
            "UPDATE billing_invoices SET updated_at = now() WHERE tenant_id = $1 AND id = $2",
        )
        .bind(self.tenant.as_str())
        .bind(id.as_str())
        .execute(&mut *tx)
        .await
        .map_err(StoreError::Db)?;

        tx.commit().await.map_err(StoreError::Db)?;
        Ok(())
    }

    /// What a line set **would** total, without writing anything — the same
    /// arithmetic the stored document will report, so a draft editor can show
    /// live totals from the server rather than computing money in the browser.
    ///
    /// # Errors
    /// [`StoreError::Validation`] on the same rules as
    /// [`AccountStore::set_billing_invoice_lines`].
    pub fn billing_line_totals(&self, lines: &[NewLine]) -> Result<Totals> {
        let lines = normalize_lines(lines)?;
        let figures: Vec<LineFigures> = lines.iter().map(NormalizedLine::figures).collect();
        Ok(totals(&figures))
    }
}

// ---- row types --------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct InvoiceRow {
    id: String,
    customer_id: String,
    status: String,
    currency: String,
    number: Option<String>,
    issue_date: Option<Date>,
    due_date: Option<Date>,
    payment_terms_days: i32,
    is_credit_note: bool,
    credits_invoice_id: Option<String>,
    reference: String,
    note: String,
    created_by: String,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl InvoiceRow {
    /// A status the code does not know is corrupt data, not user input: it is
    /// reported as a decode failure (detail in the source, never in the
    /// message) rather than guessed at, because guessing here would mean
    /// treating a frozen document as editable.
    fn into_invoice(self) -> Result<Invoice> {
        let status = InvoiceStatus::parse(&self.status).ok_or_else(|| {
            StoreError::Db(sqlx::Error::Decode(
                "billing_invoices.status is not a known status".into(),
            ))
        })?;
        Ok(Invoice {
            id: BillingInvoiceId::new(self.id),
            customer_id: BillingCustomerId::new(self.customer_id),
            status,
            currency: self.currency,
            number: self.number,
            issue_date: self.issue_date,
            due_date: self.due_date,
            payment_terms_days: self.payment_terms_days,
            is_credit_note: self.is_credit_note,
            credits_invoice_id: self.credits_invoice_id.map(BillingInvoiceId::new),
            reference: self.reference,
            note: self.note,
            created_by: self.created_by,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

#[derive(sqlx::FromRow)]
struct LineRow {
    id: String,
    line_order: i32,
    description: String,
    unit: String,
    qty_milli: i64,
    unit_price_cents: i64,
    vat_rate_bp: i32,
}

impl LineRow {
    fn into_line(self) -> Line {
        Line {
            id: BillingLineId::new(self.id),
            line_order: self.line_order,
            description: self.description,
            unit: self.unit,
            qty_milli: self.qty_milli,
            unit_price_cents: self.unit_price_cents,
            vat_rate_bp: self.vat_rate_bp,
        }
    }
}

/// Just the numbers, for the list surface: the totals of many documents
/// without dragging every description over the wire.
#[derive(sqlx::FromRow)]
struct FiguresRow {
    invoice_id: String,
    qty_milli: i64,
    unit_price_cents: i64,
    vat_rate_bp: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_status_round_trips_through_its_stored_form() {
        for status in [
            InvoiceStatus::Draft,
            InvoiceStatus::Issued,
            InvoiceStatus::Paid,
            InvoiceStatus::Void,
        ] {
            assert_eq!(InvoiceStatus::parse(status.as_str()), Some(status));
        }
        assert!(InvoiceStatus::Draft.is_draft());
        for other in [
            InvoiceStatus::Issued,
            InvoiceStatus::Paid,
            InvoiceStatus::Void,
        ] {
            assert!(!other.is_draft());
        }
    }

    #[test]
    fn an_unknown_stored_status_is_never_guessed_at() {
        // Including near-misses: a document that says "Draft" or "sent" is
        // corrupt data, and treating it as a draft would make a frozen
        // document editable.
        for bad in ["", "Draft", "DRAFT", "sent", "cancelled", "issued "] {
            assert_eq!(InvoiceStatus::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_new_invoice_defaults_to_the_customers_own_terms() {
        let input = NewInvoice::for_customer(BillingCustomerId::new("cust"));
        assert!(
            input.currency.is_none() && input.payment_terms_days.is_none(),
            "None means: take the customer's"
        );
        assert!(input.reference.is_empty() && input.note.is_empty());
    }
}
