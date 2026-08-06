// What the invoice screens promise, proven against a recorded network: that
// every figure on them is one the server sent, that a typed quantity reaches
// the API as milli-units and a typed price as integer cents, that a row which
// is not yet a line stops the save instead of being dropped from it, and that
// a document carrying a number offers no edits at all.
//
// Only the network is fake. The real router, the real module routes, the real
// client and the real line model all run — a test that stubbed those would be
// testing a drawing of the editor.
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import { DialogProvider } from "../ds";
import { strings } from "../i18n";
import { BillingModule } from "./BillingModule";
import type { BillingCustomer, BillingInvoice, BillingProduct } from "./types";

interface Call {
  url: string;
  method: string;
  body: unknown;
}

interface Reply {
  match: (url: string, method: string) => boolean;
  status: number;
  body: unknown;
}

const calls: Call[] = [];
/** Answers queued for specific requests; the first match wins and is spent. */
let replies: Reply[] = [];

/** Queues one answer for the next request whose URL contains `urlPart`. */
function reply(urlPart: string, method: string, body: unknown, status = 200) {
  replies.push({ match: (url, m) => url.includes(urlPart) && m === method, status, body });
}

const CUSTOMER: BillingCustomer = {
  id: "c-1",
  name: "Acme GmbH",
  addressLine1: "Hauptstrasse 1",
  addressLine2: "",
  postalCode: "20095",
  city: "Hamburg",
  country: "DE",
  vatId: "DE811907980",
  email: "billing@acme.test",
  paymentTermsDays: 14,
  currency: "EUR",
  contactId: null,
  archived: false,
  archivedAt: null,
  createdBy: "u-1",
  createdAt: "2026-08-06T10:00:00Z",
  updatedAt: "2026-08-06T10:00:00Z",
};

const PRODUCT: BillingProduct = {
  id: "p-1",
  name: "Consulting",
  unit: "hour",
  unitPriceCents: 12500,
  vatRateBp: 2100,
  archived: false,
  archivedAt: null,
  createdBy: "u-1",
  createdAt: "2026-08-06T10:00:00Z",
  updatedAt: "2026-08-06T10:00:00Z",
};

const DRAFT: BillingInvoice = {
  id: "inv-1",
  customerId: "c-1",
  status: "draft",
  currency: "EUR",
  number: null,
  issueDate: null,
  dueDate: null,
  paymentTermsDays: 14,
  overdue: false,
  creditNote: false,
  creditsInvoiceId: null,
  quoteId: null,
  reference: "PO-77",
  note: "",
  createdBy: "u-1",
  createdAt: "2026-08-06T10:00:00Z",
  updatedAt: "2026-08-06T10:00:00Z",
  lines: [
    {
      id: "l-1",
      description: "Consulting",
      unit: "hour",
      qtyMilli: 1500,
      unitPriceCents: 12500,
      vatRateBp: 2100,
      netCents: 18750,
    },
  ],
  totals: {
    netCents: 18750,
    vatCents: 3938,
    grossCents: 22688,
    vatByRate: [{ rateBp: 2100, netCents: 18750, vatCents: 3938 }],
  },
};

const ISSUED: BillingInvoice = {
  ...DRAFT,
  id: "inv-2",
  status: "issued",
  number: "INV-2026-00007",
  issueDate: "2026-07-01",
  dueDate: "2026-07-15",
  overdue: true,
};

const fakeFetch = vi.fn(async (url: string, init?: RequestInit) => {
  const method = init?.method ?? "GET";
  calls.push({
    url,
    method,
    body: typeof init?.body === "string" ? JSON.parse(init.body) : undefined,
  });
  const index = replies.findIndex((r) => r.match(url, method));
  const answer = index === -1 ? fallback(url, method) : (replies.splice(index, 1)[0] as Reply);
  return new Response(JSON.stringify(answer.body), {
    status: answer.status,
    headers: { "content-type": "application/json" },
  });
});

/** The lists a screen loads before anything interesting happens. */
function fallback(url: string, method: string): Reply {
  const body =
    method !== "GET"
      ? {}
      : url.includes("/billing/customers")
        ? { customers: [CUSTOMER] }
        : url.includes("/billing/products")
          ? { products: [PRODUCT] }
          : { invoices: [] };
  return { match: () => true, status: 200, body };
}

vi.mock("../auth", () => ({
  useAuth: () => ({ authorizedFetch: fakeFetch }),
}));

/** The module as it is really mounted: at `/billing/*`, routing itself. */
function ui(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <DialogProvider>
        <Routes>
          <Route path="/billing/*" element={<BillingModule />} />
        </Routes>
      </DialogProvider>
    </MemoryRouter>,
  );
}

/** The last write the client made, if it made one. */
function lastWrite(): Call | undefined {
  return calls.filter((c) => c.method !== "GET").at(-1);
}

beforeEach(() => {
  calls.length = 0;
  replies = [];
  fakeFetch.mockClear();
});

afterEach(cleanup);

describe("the invoice list", () => {
  test("shows the server's number, customer and total, and marks what is late", async () => {
    reply("/billing/invoices", "GET", { invoices: [ISSUED] });
    ui("/billing/invoices");

    expect(await screen.findByText("INV-2026-00007")).toBeTruthy();
    const row = within(screen.getByRole("table"));
    expect(row.getByText("Acme GmbH")).toBeTruthy();
    // €226.88 is the server's gross; nothing here adds up the lines.
    expect(row.getByText("€226.88")).toBeTruthy();
    // The chips, not the filter's options, which carry the same words.
    expect(row.getByText(strings.billingStatusIssued)).toBeTruthy();
    expect(row.getByText(strings.billingStatusOverdue)).toBeTruthy();
    expect(row.getByText("Jul 1, 2026")).toBeTruthy();
  });

  test("the status filter asks the server, rather than filtering a loaded page", async () => {
    reply("/billing/invoices", "GET", { invoices: [DRAFT] });
    ui("/billing/invoices");
    await screen.findByText(strings.billingStatusDraft);

    reply("/billing/invoices", "GET", { invoices: [ISSUED] });
    fireEvent.change(screen.getByLabelText(strings.billingFilterStatus, { exact: false }), {
      target: { value: "issued" },
    });

    await waitFor(() =>
      expect(calls.some((c) => c.url.includes("/billing/invoices?status=issued"))).toBe(true),
    );
  });
});

describe("raising a draft", () => {
  test("a draft is raised for the chosen customer, and nothing else is sent", async () => {
    ui("/billing/invoices/new");

    const picker = await screen.findByLabelText(strings.billingFieldCustomer);
    fireEvent.change(picker, { target: { value: "c-1" } });
    reply("/billing/invoices", "POST", { invoice: { ...DRAFT, lines: [], reference: "" } });
    fireEvent.click(screen.getByRole("button", { name: strings.billingCreateDraft }));

    await waitFor(() => expect(lastWrite()).toBeTruthy());
    const write = lastWrite();
    expect(write?.method).toBe("POST");
    expect(write?.url).toContain("/billing/invoices");
    // No lines, no totals, no number — a draft is raised, then filled in —
    // and the blanks are absent, so the customer's own currency and payment
    // term still apply.
    expect(write?.body).toEqual({ customerId: "c-1" });
  });
});

describe("the draft editor", () => {
  test("a typed quantity is saved as milli-units and the new totals are the server's", async () => {
    reply("/billing/invoices/inv-1", "GET", { invoice: DRAFT, creditNotes: [] });
    ui("/billing/invoices/inv-1");

    // What the API said this draft is worth, before anything is touched.
    expect(await screen.findByText("€226.88")).toBeTruthy();

    reply("/billing/invoices/inv-1", "PATCH", {
      // Deliberately not what the lines multiply out to: whatever the server
      // says a document is worth is what the screen must show.
      invoice: {
        ...DRAFT,
        lines: [{ ...DRAFT.lines[0]!, qtyMilli: 2000, netCents: 25000 }],
        totals: {
          netCents: 25000,
          vatCents: 5250,
          grossCents: 99999,
          vatByRate: [{ rateBp: 2100, netCents: 25000, vatCents: 5250 }],
        },
      },
    });
    fireEvent.change(screen.getByLabelText(strings.billingColQty), { target: { value: "2" } });

    await waitFor(() => expect(lastWrite()).toBeTruthy(), { timeout: 3000 });
    const write = lastWrite();
    expect(write?.method).toBe("PATCH");
    // Only the line set: nothing in the header changed. Restating the customer
    // would send the document back through the store's customer check on every
    // save, and a draft raised for a since-archived customer would then be
    // uneditable — proven on the wire, not guessed at.
    expect(write?.body).toEqual({
      lines: [
        {
          description: "Consulting",
          unit: "hour",
          qtyMilli: 2000,
          unitPriceCents: 12500,
          vatRateBp: 2100,
        },
      ],
    });
    expect(await screen.findByText("€999.99")).toBeTruthy();
  });

  test("a changed header field is stated, and only that one", async () => {
    reply("/billing/invoices/inv-1", "GET", { invoice: DRAFT, creditNotes: [] });
    ui("/billing/invoices/inv-1");
    await screen.findByText("€226.88");

    reply("/billing/invoices/inv-1", "PATCH", { invoice: DRAFT });
    fireEvent.change(screen.getByPlaceholderText(strings.billingReferencePlaceholder), {
      target: { value: "PO-88" },
    });

    await waitFor(() => expect(lastWrite()).toBeTruthy(), { timeout: 3000 });
    const body = lastWrite()?.body as Record<string, unknown>;
    expect(body.reference).toBe("PO-88");
    expect(body.customerId).toBeUndefined();
    expect(body.note).toBeUndefined();
  });

  test("a price typed in any European notation reaches the API as integer cents", async () => {
    reply("/billing/invoices/inv-1", "GET", { invoice: DRAFT, creditNotes: [] });
    ui("/billing/invoices/inv-1");
    await screen.findByText("€226.88");

    reply("/billing/invoices/inv-1", "PATCH", { invoice: DRAFT });
    fireEvent.change(screen.getByLabelText(strings.billingColUnitPrice), {
      target: { value: "1 234,56" },
    });

    await waitFor(() => expect(lastWrite()).toBeTruthy(), { timeout: 3000 });
    expect(lastWrite()?.body).toHaveProperty("lines.0.unitPriceCents", 123456);
  });

  test("a row that is not a line yet stops the save instead of being dropped", async () => {
    reply("/billing/invoices/inv-1", "GET", { invoice: DRAFT, creditNotes: [] });
    ui("/billing/invoices/inv-1");
    await screen.findByText("€226.88");

    fireEvent.click(screen.getByRole("button", { name: strings.billingAddLine }));
    const prices = screen.getAllByLabelText(strings.billingColUnitPrice);
    fireEvent.change(prices[1]!, { target: { value: "50" } });

    expect(await screen.findByText(strings.billingLineNeedsDescription)).toBeTruthy();
    // Long enough that a debounce would have fired twice over.
    await new Promise((done) => setTimeout(done, 1500));
    expect(lastWrite()).toBeUndefined();
    expect(screen.getByText(strings.billingUnsaved)).toBeTruthy();
  });

  test("picking a price-list item copies its price and rate onto the line", async () => {
    reply("/billing/invoices/inv-1", "GET", { invoice: { ...DRAFT, lines: [] }, creditNotes: [] });
    ui("/billing/invoices/inv-1");
    await screen.findByRole("button", { name: strings.billingAddLine });

    fireEvent.click(screen.getByRole("button", { name: strings.billingAddLine }));
    reply("/billing/invoices/inv-1", "PATCH", { invoice: DRAFT });
    fireEvent.change(screen.getByLabelText(strings.billingPickProduct), {
      target: { value: "p-1" },
    });

    await waitFor(() => expect(lastWrite()).toBeTruthy(), { timeout: 3000 });
    expect(lastWrite()?.body).toHaveProperty("lines.0", {
      description: "Consulting",
      unit: "hour",
      // Nobody said how many, so the line bills one.
      qtyMilli: 1000,
      unitPriceCents: 12500,
      vatRateBp: 2100,
    });
  });

  test("a refusal is shown in the server's own words and nothing is lost", async () => {
    reply("/billing/invoices/inv-1", "GET", { invoice: DRAFT, creditNotes: [] });
    ui("/billing/invoices/inv-1");
    await screen.findByText("€226.88");

    reply("/billing/invoices/inv-1", "PATCH", { detail: "a line needs a description" }, 422);
    fireEvent.change(screen.getByLabelText(strings.billingColQty), { target: { value: "3" } });

    expect(await screen.findByRole("alert")).toHaveProperty(
      "textContent",
      "a line needs a description",
    );
    expect((screen.getByLabelText(strings.billingColQty) as HTMLInputElement).value).toBe("3");
  });

  test("a document that carries a number offers no edits", async () => {
    reply("/billing/invoices/inv-2", "GET", { invoice: ISSUED, creditNotes: [] });
    ui("/billing/invoices/inv-2");

    expect(await screen.findByText(strings.billingFrozenNotice)).toBeTruthy();
    expect(screen.queryByLabelText(strings.billingColQty)).toBeNull();
    expect(screen.queryByRole("button", { name: strings.billingAddLine })).toBeNull();
    expect(screen.queryByRole("button", { name: strings.billingDeleteDraft })).toBeNull();
    // The stored line, formatted from the document rather than from a form.
    const table = screen.getByRole("table");
    expect(within(table).getByText("1.5")).toBeTruthy();
    expect(within(table).getByText("21%")).toBeTruthy();
  });
});
