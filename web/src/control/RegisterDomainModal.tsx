// Register a domain to a tenant (control plane). After registration the modal
// shows the exact DNS TXT record to publish; the operator publishes it and
// then verifies from the domain row. A domain must be verified before its
// tenant can assign addresses in it.
import { useState } from "react";
import type { FormEvent } from "react";
import { X } from "lucide-react";

import { strings } from "../i18n";
import { Button, Spinner } from "../ds";
import { useJmapClient } from "../jmap";
import type { ControlDomain, ControlTenant } from "../jmap";
import styles from "../admin/admin.module.css";

interface Props {
  tenants: ControlTenant[];
  onClose: () => void;
  onRegistered: () => void;
}

export function RegisterDomainModal({ tenants, onClose, onRegistered }: Props) {
  const client = useJmapClient();
  const [tenantId, setTenantId] = useState(tenants[0]?.id ?? "");
  const [domain, setDomain] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [registered, setRegistered] = useState<ControlDomain | null>(null);

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (tenantId === "" || !domain.includes(".")) {
      setError(strings.domainInvalid);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const row = await client.createDomain(tenantId, domain.trim().toLowerCase());
      setRegistered(row);
    } catch {
      setError(strings.domainCreateError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className={styles.overlay} onMouseDown={onClose}>
      <div
        className={styles.modal}
        role="dialog"
        aria-modal="true"
        aria-label={strings.domainAdd}
        onMouseDown={(e) => e.stopPropagation()}
      >
        {registered === null ? (
          <form onSubmit={submit}>
            <div className={styles.modalHead}>
              <h2>{strings.domainAdd}</h2>
              <button type="button" className={styles.iconBtn} onClick={onClose} aria-label={strings.userClose}>
                <X size={18} />
              </button>
            </div>
            <div className={styles.modalBody}>
              <label className={styles.field}>
                <span className={styles.label}>{strings.domainTenant}</span>
                <select
                  className={styles.input}
                  value={tenantId}
                  onChange={(e) => setTenantId(e.target.value)}
                >
                  {tenants.map((t) => (
                    <option key={t.id} value={t.id}>
                      {t.name}
                    </option>
                  ))}
                </select>
              </label>
              <label className={styles.field}>
                <span className={styles.label}>{strings.domainName}</span>
                <input
                  className={styles.input}
                  value={domain}
                  onChange={(e) => setDomain(e.target.value)}
                  placeholder="customer.example"
                  autoFocus
                />
              </label>
              {error !== null && (
                <p className={styles.error} role="alert">
                  {error}
                </p>
              )}
            </div>
            <div className={styles.modalFoot}>
              <div className={styles.footSpacer} />
              <button type="button" className={styles.textBtn} onClick={onClose}>
                {strings.providerCancel}
              </button>
              <Button type="submit" disabled={busy}>
                {busy ? <Spinner size={16} /> : strings.domainRegister}
              </Button>
            </div>
          </form>
        ) : (
          <>
            <div className={styles.modalHead}>
              <h2>{strings.domainPublishTitle}</h2>
              <button type="button" className={styles.iconBtn} onClick={onClose} aria-label={strings.userClose}>
                <X size={18} />
              </button>
            </div>
            <div className={styles.modalBody}>
              <p className={styles.pageIntro}>{strings.domainPublishIntro(registered.domain)}</p>
              <label className={styles.field}>
                <span className={styles.label}>{strings.domainRecordName}</span>
                <input className={styles.input} readOnly value={registered.verifyRecord.name} />
              </label>
              <label className={styles.field}>
                <span className={styles.label}>{strings.domainRecordType}</span>
                <input className={styles.input} readOnly value={registered.verifyRecord.type} />
              </label>
              <label className={styles.field}>
                <span className={styles.label}>{strings.domainRecordValue}</span>
                <input className={styles.input} readOnly value={registered.verifyRecord.value} />
              </label>
            </div>
            <div className={styles.modalFoot}>
              <div className={styles.footSpacer} />
              <button type="button" className={styles.primary} onClick={onRegistered}>
                {strings.domainPublishDone}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
