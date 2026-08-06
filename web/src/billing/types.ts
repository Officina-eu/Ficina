// The JSON shapes the `/billing` HTTP surface speaks (alo Billing, ADR 0035,
// wave B1). These mirror `products/mail/alo-jmap/src/billing_customers.rs` and
// `…/billing_products.rs` field for field — the server is the contract, this
// file only names it for TypeScript.
//
// Two rules the types encode, because they are the ones a UI gets wrong:
//   - **Money is integer cents and rates are basis points.** `unitPriceCents`
//     and `vatRateBp` are whole numbers; a decimal sent here is a `400` from
//     the server, never a silently rounded price.
//   - **A draft is a partial write.** Every field of `CustomerDraft` /
//     `ProductDraft` is optional: an absent field keeps its stored value on a
//     `PATCH`, and takes the server's default on a create. `null` clears a
//     nullable field — which is why the nullable fields are `string | null`
//     rather than merely optional.

/** A billing customer as the server stores it. */
export interface BillingCustomer {
  id: string;
  name: string;
  addressLine1: string;
  addressLine2: string;
  postalCode: string;
  city: string;
  /** ISO 3166-1 alpha-2, uppercase — canonicalised by the server. */
  country: string;
  /** VAT identification number in canonical form; `null` for B2C. */
  vatId: string | null;
  email: string | null;
  paymentTermsDays: number;
  /** ISO 4217, uppercase. */
  currency: string;
  /** Linked address-book contact, if any. */
  contactId: string | null;
  archived: boolean;
  archivedAt: string | null;
  createdBy: string;
  createdAt: string;
  updatedAt: string;
}

/** The writable fields of a customer; absent means "leave as it is". */
export interface CustomerDraft {
  name?: string;
  addressLine1?: string;
  addressLine2?: string;
  postalCode?: string;
  city?: string;
  country?: string;
  vatId?: string | null;
  email?: string | null;
  paymentTermsDays?: number;
  currency?: string;
  contactId?: string | null;
}

/** A price-list item as the server stores it. No currency: a price list is
 *  quoted in the tenant's own currency, and a document carries the currency it
 *  was raised in (`docs/design/billing.md`). */
export interface BillingProduct {
  id: string;
  name: string;
  /** Unit label; empty for a unitless item. */
  unit: string;
  unitPriceCents: number;
  vatRateBp: number;
  archived: boolean;
  archivedAt: string | null;
  createdBy: string;
  createdAt: string;
  updatedAt: string;
}

/** The writable fields of a product; absent means "leave as it is". */
export interface ProductDraft {
  name?: string;
  unit?: string;
  unitPriceCents?: number;
  vatRateBp?: number;
}
