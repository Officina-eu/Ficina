// How a document's state is shown — one place, so a chip in the list and a
// chip on the editor can never say different things about the same invoice.
//
// The state a reader cares about is not exactly the stored one: an issued
// invoice past its due date is "overdue", which the server computes against
// its own date (`overdue` on every invoice response) rather than the browser's.
// So the chip a row shows is derived from the pair, never from `status` alone.
import { cx } from "../ds";
import { strings } from "../i18n";
import type { BillingInvoiceSummary, InvoiceStatus } from "./types";
import styles from "./BillingModule.module.css";

/** The visual weight of a chip. Named after what it means, not its colour, so
 *  a theme can restyle it without renaming anything. */
export type ChipTone = "neutral" | "info" | "good" | "warn" | "muted";

/** A small state label. */
export function StatusChip({ tone, label }: { tone: ChipTone; label: string }) {
  return <span className={cx(styles.chip, styles[`chip_${tone}`])}>{label}</span>;
}

/** What to call a status. An unknown one — a state added to the server before
 *  this client knows it — is shown verbatim rather than blanked. */
export function statusLabel(status: InvoiceStatus): string {
  switch (status) {
    case "draft":
      return strings.billingStatusDraft;
    case "issued":
      return strings.billingStatusIssued;
    case "paid":
      return strings.billingStatusPaid;
    case "void":
      return strings.billingStatusVoid;
    default:
      return status;
  }
}

/** How loudly a status reads: a draft is quiet, money in is good, a cancelled
 *  document is greyed out. */
export function statusTone(status: InvoiceStatus): ChipTone {
  switch (status) {
    case "issued":
      return "info";
    case "paid":
      return "good";
    case "void":
      return "muted";
    default:
      return "neutral";
  }
}

/** The chips one document wears, in reading order: what it is, then what is
 *  wrong with it. A credit note says so first — it is the one thing about a
 *  document a reader must not miss, because its totals are negative. */
export function DocumentChips({ invoice }: { invoice: BillingInvoiceSummary }) {
  return (
    <>
      {invoice.creditNote && <StatusChip tone="warn" label={strings.billingCreditNote} />}
      <StatusChip tone={statusTone(invoice.status)} label={statusLabel(invoice.status)} />
      {invoice.overdue && <StatusChip tone="warn" label={strings.billingStatusOverdue} />}
    </>
  );
}
