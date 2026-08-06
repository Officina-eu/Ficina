//! Field rules shared by every alo Billing record (ADR 0035, wave B1).
//!
//! Customers, products, invoice lines and quote lines all bound the same
//! handful of primitive shapes — a trimmed string, a required name, an amount
//! in integer cents, a VAT rate in basis points. They live here so a rule is
//! stated once and every billing module answers a caller identically; a
//! module keeps only the rules that are genuinely its own (a customer's VAT
//! id, a product's unit).
//!
//! Every function is pure and returns [`StoreError::Validation`] naming the
//! violated rule — never echoing the value, which may be customer data
//! (law 1).

use crate::error::{Result, StoreError};

/// The highest VAT rate we accept, in basis points: 10 000 bp = 100 %.
///
/// No member state levies anywhere near this, but the ceiling is the one that
/// is *definitionally* true rather than a guess at fiscal policy, so a rate
/// change in any member state can never make us reject a real invoice.
pub const VAT_RATE_MAX_BP: i32 = 10_000;

/// The highest unit price we accept, in cents: €10 000 000.00 per unit.
///
/// This is a typo guard with an arithmetic job. Line net is
/// `qty_milli × unit_price_cents / 1000` (B1.06); capping the price at 10^9
/// cents keeps that product inside `i64` for any quantity the line model can
/// hold, so no document total can ever overflow into a wrong number.
pub const UNIT_PRICE_MAX_CENTS: i64 = 1_000_000_000;

/// Trims `value` and rejects it if it exceeds `max` characters.
///
/// Counts characters, not bytes: a 200-character limit means 200 characters
/// of any script, so a name in Greek is not half the length of one in ASCII.
pub(crate) fn bounded(field: &str, value: &str, max: usize) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.chars().count() > max {
        return Err(StoreError::Validation(format!(
            "{field} must be at most {max} characters"
        )));
    }
    Ok(trimmed.to_owned())
}

/// [`bounded`], and additionally non-blank — for a field the record cannot
/// meaningfully exist without (a customer's name, a product's name).
pub(crate) fn required(field: &str, value: &str, max: usize) -> Result<String> {
    let value = bounded(field, value, max)?;
    if value.is_empty() {
        return Err(StoreError::Validation(format!("{field} must not be empty")));
    }
    Ok(value)
}

/// Validates a VAT rate in basis points (2100 = 21 %). Zero is valid and
/// common: exempt, reverse-charge and intra-Community supplies are all 0 %.
pub(crate) fn vat_rate_bp(value: i32) -> Result<i32> {
    if !(0..=VAT_RATE_MAX_BP).contains(&value) {
        return Err(StoreError::Validation(format!(
            "VAT rate must be between 0 and {VAT_RATE_MAX_BP} basis points"
        )));
    }
    Ok(value)
}

/// Validates a non-negative amount in integer cents against
/// [`UNIT_PRICE_MAX_CENTS`].
///
/// Negative prices are refused here on purpose: a discount is a negative
/// *quantity* or a credit note (B1.09), both of which stay auditable, whereas
/// a negative price hides a refund inside an ordinary invoice line.
pub(crate) fn unit_price_cents(field: &str, value: i64) -> Result<i64> {
    if !(0..=UNIT_PRICE_MAX_CENTS).contains(&value) {
        return Err(StoreError::Validation(format!(
            "{field} must be between 0 and {UNIT_PRICE_MAX_CENTS} cents"
        )));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message<T: std::fmt::Debug>(result: Result<T>) -> String {
        match result {
            Err(StoreError::Validation(msg)) => msg,
            other => panic!("expected Validation, got {other:?}"),
        }
    }

    #[test]
    fn bounded_trims_and_counts_characters_not_bytes() {
        assert_eq!(bounded("name", "  Acme  ", 10).unwrap_or_default(), "Acme");
        // Five two-byte characters fit a five-character bound.
        assert_eq!(bounded("name", "Ωμέγα", 5).unwrap_or_default(), "Ωμέγα");
        assert!(message(bounded("name", "Ωμέγα", 4)).contains("at most 4"));
        // Trimming happens before measuring: padding never costs a caller.
        assert!(bounded("name", "    abc    ", 3).is_ok());
    }

    #[test]
    fn required_rejects_blank_but_keeps_the_bound() {
        for blank in ["", "   ", "\t\n"] {
            assert!(message(required("name", blank, 10)).contains("must not be empty"));
        }
        assert!(message(required("name", "abcdef", 3)).contains("at most"));
        assert_eq!(required("name", " ok ", 10).unwrap_or_default(), "ok");
    }

    #[test]
    fn vat_rate_spans_zero_to_one_hundred_percent() {
        for ok in [0, 600, 2100, VAT_RATE_MAX_BP] {
            assert_eq!(vat_rate_bp(ok).unwrap_or_default(), ok);
        }
        for bad in [-1, VAT_RATE_MAX_BP + 1, i32::MIN, i32::MAX] {
            assert!(
                matches!(vat_rate_bp(bad), Err(StoreError::Validation(_))),
                "expected rejection: {bad}"
            );
        }
    }

    #[test]
    fn unit_price_is_non_negative_and_capped() {
        for ok in [0, 1, 1_250, UNIT_PRICE_MAX_CENTS] {
            assert_eq!(unit_price_cents("unit price", ok).unwrap_or_default(), ok);
        }
        for bad in [-1, UNIT_PRICE_MAX_CENTS + 1, i64::MIN, i64::MAX] {
            assert!(
                matches!(
                    unit_price_cents("unit price", bad),
                    Err(StoreError::Validation(_))
                ),
                "expected rejection: {bad}"
            );
        }
    }

    #[test]
    fn the_price_cap_keeps_line_arithmetic_inside_i64() {
        // B1.06 computes `qty_milli × unit_price_cents`. At the ceiling of
        // both, that product must still be an i64 — otherwise a total could
        // silently wrap. A million units of the dearest possible item:
        let qty_milli: i64 = 1_000_000_000;
        assert!(qty_milli.checked_mul(UNIT_PRICE_MAX_CENTS).is_some());
    }
}
