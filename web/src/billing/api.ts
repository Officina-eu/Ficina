// The client for the `/billing` HTTP surface (alo Billing, ADR 0035, wave B1).
//
// Deliberately its own small client rather than more methods on `JmapClient`:
// billing is a plain REST surface with none of JMAP's session, capabilities or
// method-call envelope, and it changes for entirely different reasons than
// mail does. It uses the same authenticated fetch (bearer + refresh handled by
// the auth layer), so there is one session, not two.
//
// It holds NO validation. Name, country, currency, VAT id, price and rate are
// all ruled on by the store, which the billing agent (B1.25) also calls
// directly; a second, weaker copy of those rules here is exactly how the two
// doors end up disagreeing. The form's job is to send what was typed and show
// what came back.
import { useMemo } from "react";

import { useAuth } from "../auth";
import { API_BASE } from "../platform/runtime";
import type {
  BillingCustomer,
  BillingProduct,
  CustomerDraft,
  ProductDraft,
} from "./types";

type AuthorizedFetch = (input: string, init?: RequestInit) => Promise<Response>;

/**
 * A failed billing request. `detail` is the server's own sentence when it sent
 * one — the store authors those messages to name the rule that was broken and
 * never to echo stored data, so they are safe to put in front of a user.
 * `status` lets a caller tell "you typed something impossible" (422) from
 * "that record is gone" (404) without parsing prose.
 */
export class BillingError extends Error {
  readonly status: number;
  readonly detail: string | null;

  constructor(status: number, detail: string | null) {
    super(detail ?? `billing request failed (${status})`);
    this.name = "BillingError";
    this.status = status;
    this.detail = detail;
  }
}

/**
 * What to show a user about a failed request: the server's own sentence when
 * it sent one, and `fallback` otherwise (a dropped connection, or a failure
 * whose reason is not the user's business). One helper, so every billing
 * screen reports a failure the same way.
 */
export function billingMessage(error: unknown, fallback: string): string {
  return error instanceof BillingError && error.detail !== null ? error.detail : fallback;
}

/** The tenant's customers and price list. One instance per auth context. */
export class BillingApi {
  readonly #fetch: AuthorizedFetch;

  constructor(authorizedFetch: AuthorizedFetch) {
    this.#fetch = authorizedFetch;
  }

  /** The customer list, active first; archived only when asked for. */
  customers(includeArchived = false): Promise<BillingCustomer[]> {
    return this.#read<{ customers?: BillingCustomer[] }>(
      `/billing/customers${includeArchived ? "?includeArchived=1" : ""}`,
    ).then((r) => r.customers ?? []);
  }

  /** Creates a customer; answers the STORED record, which is canonicalised
   *  (country and currency uppercased, VAT id compacted and prefixed). */
  createCustomer(draft: CustomerDraft): Promise<BillingCustomer> {
    return this.#write<{ customer: BillingCustomer }>("POST", "/billing/customers", draft).then(
      (r) => r.customer,
    );
  }

  /** Edits a customer. Absent fields keep their stored value; `null` clears a
   *  nullable one. Last writer wins — there is no `If-Match` yet. */
  updateCustomer(id: string, draft: CustomerDraft): Promise<BillingCustomer> {
    return this.#write<{ customer: BillingCustomer }>(
      "PATCH",
      `/billing/customers/${encodeURIComponent(id)}`,
      draft,
    ).then((r) => r.customer);
  }

  /** Archives or restores a customer. Archiving is the only removal: an issued
   *  invoice must always be able to name who it was for. */
  setCustomerArchived(id: string, archived: boolean): Promise<BillingCustomer> {
    return this.#write<{ customer: BillingCustomer }>(
      "POST",
      `/billing/customers/${encodeURIComponent(id)}/archive`,
      { archived },
    ).then((r) => r.customer);
  }

  /** The price list, active first; archived only when asked for. */
  products(includeArchived = false): Promise<BillingProduct[]> {
    return this.#read<{ products?: BillingProduct[] }>(
      `/billing/products${includeArchived ? "?includeArchived=1" : ""}`,
    ).then((r) => r.products ?? []);
  }

  /** Creates a price-list item. */
  createProduct(draft: ProductDraft): Promise<BillingProduct> {
    return this.#write<{ product: BillingProduct }>("POST", "/billing/products", draft).then(
      (r) => r.product,
    );
  }

  /** Edits a price-list item. Never rewrites a document already raised — a
   *  line copies name, unit, price and rate at the moment it is picked. */
  updateProduct(id: string, draft: ProductDraft): Promise<BillingProduct> {
    return this.#write<{ product: BillingProduct }>(
      "PATCH",
      `/billing/products/${encodeURIComponent(id)}`,
      draft,
    ).then((r) => r.product);
  }

  /** Archives or restores a price-list item. */
  setProductArchived(id: string, archived: boolean): Promise<BillingProduct> {
    return this.#write<{ product: BillingProduct }>(
      "POST",
      `/billing/products/${encodeURIComponent(id)}/archive`,
      { archived },
    ).then((r) => r.product);
  }

  async #read<T>(path: string): Promise<T> {
    return this.#json<T>(await this.#send(path, { method: "GET" }));
  }

  async #write<T>(method: string, path: string, body: unknown): Promise<T> {
    return this.#json<T>(
      await this.#send(path, {
        method,
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body),
      }),
    );
  }

  async #send(path: string, init: RequestInit): Promise<Response> {
    try {
      return await this.#fetch(`${API_BASE}${path}`, init);
    } catch {
      // A dropped connection is not a status code; give it one the UI can
      // treat like any other failure rather than an unhandled rejection.
      throw new BillingError(0, null);
    }
  }

  async #json<T>(res: Response): Promise<T> {
    if (!res.ok) {
      const problem = (await res.json().catch(() => ({}))) as { detail?: unknown };
      const detail = typeof problem.detail === "string" ? problem.detail : null;
      throw new BillingError(res.status, detail);
    }
    return (await res.json()) as T;
  }
}

/** The billing client bound to the current session. Memoized per auth context,
 *  so a re-render never re-creates it and effects keyed on it do not loop. */
export function useBillingApi(): BillingApi {
  const { authorizedFetch } = useAuth();
  return useMemo(() => new BillingApi(authorizedFetch), [authorizedFetch]);
}
