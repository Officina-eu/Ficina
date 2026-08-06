// The invoice editor: one document, from the customer it is raised for to the
// lines it bills and what the server says they come to.
//
// Three decisions worth knowing before reading it.
//
// - **The totals on screen are always a server response.** The draft saves
//   itself a moment after typing stops, and the document that comes back is
//   what the totals panel and the per-line nets render. Between a keystroke
//   and that response the figures are the previous ones, dimmed and labelled
//   as such — the browser never fills the gap with arithmetic of its own.
// - **A draft is raised before it is lined.** Creating one only needs a
//   customer, because the currency and the payment term are read from that
//   customer and snapshotted onto the document; lines are then edited on the
//   saved draft, which is also what makes autosave possible at all.
// - **Only a draft is editable.** A document that carries a number is frozen
//   by the store, so this screen renders it read-only rather than offering
//   edits the server would refuse. Issuing, crediting and printing are the
//   next items (B1.15–B1.16); until they land, a numbered document is a
//   record to read.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { ArrowLeft } from "lucide-react";

import { Button, Spinner, cx, useDialogs } from "../ds";
import { strings, useLocale } from "../i18n";
import { BillingError, billingMessage, useBillingApi } from "./api";
import { formatDocumentDate } from "./dates";
import { InvoiceLines } from "./InvoiceLines";
import { rowFromLine, rowsDraft } from "./lineRows";
import type { LineRow } from "./lineRows";
import { ErrorBanner, Field } from "./parts";
import { DocumentChips } from "./status";
import { TotalsPanel } from "./TotalsPanel";
import type {
  BillingCustomer,
  BillingInvoice,
  BillingInvoiceSummary,
  BillingProduct,
  InvoiceDraft,
} from "./types";
import styles from "./BillingModule.module.css";

/** How long typing has to stop before the draft saves itself. Long enough not
 *  to write a document per keystroke, short enough that the totals feel like
 *  they belong to what is on screen. */
const AUTOSAVE_MS = 700;

/** The header fields a person edits. The currency is not among them: it was
 *  snapshotted from the customer when the document was raised, and changing
 *  what a document is denominated in is not a text box. */
interface Header {
  customerId: string;
  reference: string;
  note: string;
}

/** Where the draft stands against the server. */
type SaveState = "saved" | "pending" | "saving" | "failed";

export function InvoiceEditor() {
  const { id } = useParams<{ id: string }>();
  const api = useBillingApi();
  const locale = useLocale();
  const navigate = useNavigate();
  const { confirm } = useDialogs();

  const [customers, setCustomers] = useState<BillingCustomer[]>([]);
  const [products, setProducts] = useState<BillingProduct[]>([]);
  const [invoice, setInvoice] = useState<BillingInvoice | null>(null);
  const [creditNotes, setCreditNotes] = useState<BillingInvoiceSummary[]>([]);
  const [header, setHeader] = useState<Header>({ customerId: "", reference: "", note: "" });
  const [rows, setRows] = useState<LineRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [missing, setMissing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saveState, setSaveState] = useState<SaveState>("saved");
  const [creating, setCreating] = useState(false);

  // What the autosave loop reads. State would be a render behind it, and a
  // save that sent a stale line set would silently undo a keystroke.
  const editRef = useRef<{ header: Header; rows: LineRow[] }>({ header, rows });
  /** The document as the server last stored it — what a save is diffed
   *  against, kept in a ref for the same reason the edits are. */
  const savedRef = useRef<BillingInvoice | null>(null);
  const savingRef = useRef(false);
  /** Bumped on every edit, so a save that finishes into a changed form knows
   *  to go round again instead of reporting "saved". */
  const editSeq = useRef(0);
  /** Row identity for rows that are not stored lines yet. */
  const keySeq = useRef(0);
  const nextKey = useCallback(() => {
    keySeq.current += 1;
    return `new-${keySeq.current}`;
  }, []);

  const readOnly = invoice !== null && invoice.status !== "draft";

  /** Adopts a document from the server as the form's starting position. */
  const adopt = useCallback((document: BillingInvoice) => {
    savedRef.current = document;
    setInvoice(document);
    const next = {
      header: {
        customerId: document.customerId,
        reference: document.reference,
        note: document.note,
      },
      rows: document.lines.map(rowFromLine),
    };
    editRef.current = next;
    setHeader(next.header);
    setRows(next.rows);
  }, []);

  useEffect(() => {
    let live = true;
    setLoading(true);
    void (async () => {
      try {
        // Archived customers are loaded too: a document already raised for one
        // must still be able to name them. The picker filters them out.
        const [people, catalogue] = await Promise.all([api.customers(true), api.products()]);
        if (!live) return;
        setCustomers(people);
        setProducts(catalogue);
        if (id !== undefined) {
          const loaded = await api.invoice(id);
          if (!live) return;
          adopt(loaded.invoice);
          setCreditNotes(loaded.creditNotes);
        }
        setError(null);
      } catch (err) {
        if (!live) return;
        setMissing(err instanceof BillingError && err.status === 404);
        setError(billingMessage(err, strings.billingLoadFailed));
      } finally {
        if (live) setLoading(false);
      }
    })();
    return () => {
      live = false;
    };
  }, [api, id, adopt]);

  /** Every edit goes through here: it keeps the ref the save loop reads in
   *  step with the state the screen renders, and marks the draft unsaved. */
  const edit = useCallback((next: Partial<{ header: Header; rows: LineRow[] }>) => {
    editRef.current = { ...editRef.current, ...next };
    if (next.header !== undefined) setHeader(next.header);
    if (next.rows !== undefined) setRows(next.rows);
    editSeq.current += 1;
    setSaveState("pending");
  }, []);

  /**
   * The body a save would send, or `null` while a row is not yet a line.
   *
   * Only the header fields that actually changed are stated. Restating the
   * customer would send the document back through the store's customer check
   * on every keystroke, and a draft raised for a customer who was archived
   * afterwards would then refuse to have its lines edited at all — a dead end
   * with no way out but deleting the draft. What a save always carries is the
   * line set, because the API replaces it whole.
   */
  const draftOf = useCallback(
    (edited: { header: Header; rows: LineRow[] }, base: BillingInvoice): InvoiceDraft | null => {
      const lines = rowsDraft(edited.rows);
      if (lines === null || edited.header.customerId === "") return null;
      const draft: InvoiceDraft = { lines };
      if (edited.header.customerId !== base.customerId) draft.customerId = edited.header.customerId;
      if (edited.header.reference !== base.reference) draft.reference = edited.header.reference;
      if (edited.header.note !== base.note) draft.note = edited.header.note;
      return draft;
    },
    [],
  );

  /**
   * Saves the draft, and keeps saving until the form stops moving under it.
   * The loop is what makes a single in-flight request safe: an edit that
   * lands mid-save is picked up on the next turn instead of racing it.
   */
  const save = useCallback(async () => {
    if (savingRef.current || id === undefined) return;
    const base = savedRef.current;
    if (base === null) return;
    savingRef.current = true;
    try {
      for (;;) {
        const seq = editSeq.current;
        const draft = draftOf(editRef.current, savedRef.current ?? base);
        if (draft === null) {
          setSaveState("pending");
          return;
        }
        setSaveState("saving");
        const saved = await api.updateInvoice(id, draft);
        savedRef.current = saved;
        setInvoice(saved);
        setError(null);
        if (editSeq.current === seq) {
          setSaveState("saved");
          return;
        }
      }
    } catch (err) {
      setError(billingMessage(err, strings.billingSaveFailed));
      setSaveState("failed");
    } finally {
      savingRef.current = false;
    }
  }, [api, id, draftOf]);

  // The debounce. A form that cannot be sent yet stays "pending" and simply
  // does not schedule anything — the reason is already on the offending row.
  useEffect(() => {
    if (saveState !== "pending" || readOnly || id === undefined || invoice === null) return;
    if (draftOf({ header, rows }, invoice) === null) return;
    const timer = setTimeout(() => void save(), AUTOSAVE_MS);
    return () => clearTimeout(timer);
  }, [saveState, header, rows, readOnly, id, invoice, draftOf, save]);

  /** Raises the draft this screen was opened to write. */
  async function create() {
    setCreating(true);
    setError(null);
    try {
      // Blanks stay absent, as everywhere in this module: an unstated field
      // takes the server's own default rather than being written as "".
      const draft: InvoiceDraft = { customerId: header.customerId };
      if (header.reference !== "") draft.reference = header.reference;
      if (header.note !== "") draft.note = header.note;
      const created = await api.createInvoice(draft);
      // Replaces the /new entry, so Back goes to the list rather than to a
      // form for a document that now exists.
      await navigate(`../${created.id}`, { replace: true });
    } catch (err) {
      setError(billingMessage(err, strings.billingSaveFailed));
    } finally {
      setCreating(false);
    }
  }

  async function discard() {
    if (
      id === undefined ||
      !(await confirm({
        title: strings.billingDeleteDraft,
        message: strings.billingDeleteDraftConfirm,
        confirmLabel: strings.billingDeleteDraft,
        danger: true,
      }))
    ) {
      return;
    }
    try {
      await api.deleteInvoice(id);
      await navigate("..");
    } catch (err) {
      setError(billingMessage(err, strings.billingSaveFailed));
    }
  }

  // An archived customer is still offered on the document already raised for
  // them — otherwise the picker would silently show no one and the next edit
  // would change who is being billed.
  const pickable = useMemo(
    () => customers.filter((c) => !c.archived || c.id === header.customerId),
    [customers, header.customerId],
  );
  const customerName = useMemo(
    () => customers.find((c) => c.id === header.customerId)?.name ?? "",
    [customers, header.customerId],
  );

  if (loading) {
    return (
      <div className={styles.page}>
        <div className={styles.loading}>
          <Spinner size={20} />
        </div>
      </div>
    );
  }

  if (missing) {
    return (
      <div className={styles.page}>
        <ErrorBanner message={strings.billingInvoiceGone} />
        <p className={styles.noMatches}>
          <button type="button" className={styles.linkAction} onClick={() => void navigate("..")}>
            {strings.billingBackToInvoices}
          </button>
        </p>
      </div>
    );
  }

  const currency = invoice?.currency ?? "";
  const saved = saveState === "saved";

  return (
    <div className={cx(styles.page, styles.editor)}>
      <div className={styles.editorHead}>
        <button type="button" className={styles.linkAction} onClick={() => void navigate("..")}>
          <ArrowLeft size={14} aria-hidden="true" /> {strings.billingBackToInvoices}
        </button>
        <h2 className={styles.editorTitle}>
          {invoice === null
            ? strings.billingNewInvoice
            : (invoice.number ?? strings.billingDraftInvoice)}
        </h2>
        {invoice !== null && (
          <span className={styles.chips}>
            <DocumentChips invoice={invoice} />
          </span>
        )}
        <span className={styles.saveState} role="status">
          {invoice === null
            ? ""
            : saveState === "saving"
              ? strings.billingSaving
              : saveState === "pending"
                ? strings.billingUnsaved
                : saveState === "failed"
                  ? strings.billingSaveNotDone
                  : strings.billingSaved}
        </span>
        {saveState === "failed" && (
          <button type="button" className={styles.linkAction} onClick={() => void save()}>
            {strings.billingSaveNow}
          </button>
        )}
        {invoice !== null && !readOnly && (
          <button type="button" className={styles.linkAction} onClick={() => void discard()}>
            {strings.billingDeleteDraft}
          </button>
        )}
      </div>

      {error !== null && <ErrorBanner message={error} />}
      {readOnly && <p className={styles.notice}>{strings.billingFrozenNotice}</p>}

      <div className={styles.editorBody}>
        <div className={styles.headerFields}>
          <Field label={strings.billingFieldCustomer} hint={strings.billingCustomerFixedHint}>
            {readOnly ? (
              <p className={styles.readOnlyValue}>{customerName}</p>
            ) : (
              <select
                className={styles.input}
                value={header.customerId}
                onChange={(e) => edit({ header: { ...header, customerId: e.target.value } })}
                aria-label={strings.billingFieldCustomer}
              >
                <option value="">{strings.billingChooseCustomer}</option>
                {pickable.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.name}
                  </option>
                ))}
              </select>
            )}
          </Field>

          <Field label={strings.billingFieldReference} hint={strings.billingReferenceHint}>
            {readOnly ? (
              <p className={styles.readOnlyValue}>{header.reference}</p>
            ) : (
              <input
                className={styles.input}
                value={header.reference}
                onChange={(e) => edit({ header: { ...header, reference: e.target.value } })}
                placeholder={strings.billingReferencePlaceholder}
              />
            )}
          </Field>

          {invoice !== null && (
            <>
              <Field label={strings.billingFieldIssueDate}>
                <p className={styles.readOnlyValue}>
                  {formatDocumentDate(invoice.issueDate, locale, strings.billingNoDate)}
                </p>
              </Field>
              <Field label={strings.billingFieldDueDate} hint={strings.billingTermsDays(invoice.paymentTermsDays)}>
                <p className={styles.readOnlyValue}>
                  {formatDocumentDate(invoice.dueDate, locale, strings.billingNoDate)}
                </p>
              </Field>
            </>
          )}
        </div>

        <Field label={strings.billingFieldNote} hint={strings.billingNoteHint}>
          {readOnly ? (
            <p className={styles.readOnlyValue}>{header.note}</p>
          ) : (
            <textarea
              className={cx(styles.input, styles.textarea)}
              value={header.note}
              rows={2}
              onChange={(e) => edit({ header: { ...header, note: e.target.value } })}
              placeholder={strings.billingNotePlaceholder}
            />
          )}
        </Field>

        {invoice === null ? (
          <div className={styles.createBar}>
            <p className={styles.hint}>{strings.billingCreateDraftHint}</p>
            <Button
              onClick={() => void create()}
              disabled={creating || header.customerId === ""}
            >
              {strings.billingCreateDraft}
            </Button>
          </div>
        ) : (
          <>
            <InvoiceLines
              rows={rows}
              products={products}
              savedLines={invoice.lines}
              saved={saved}
              currency={currency}
              readOnly={readOnly}
              onChange={(next) => edit({ rows: next })}
              nextKey={nextKey}
            />
            <TotalsPanel totals={invoice.totals} currency={currency} stale={!saved} />
            {creditNotes.length > 0 && (
              <section className={styles.lines}>
                <h2 className={styles.sectionTitle}>{strings.billingCreditNotes}</h2>
                <ul className={styles.creditList}>
                  {creditNotes.map((credit) => (
                    <li key={credit.id}>
                      <button
                        type="button"
                        className={cx(styles.rowName, styles.mono)}
                        onClick={() => void navigate(`../${credit.id}`)}
                      >
                        {credit.number ?? strings.billingDraftInvoice}
                      </button>
                      <DocumentChips invoice={credit} />
                    </li>
                  ))}
                </ul>
              </section>
            )}
          </>
        )}
      </div>
    </div>
  );
}
