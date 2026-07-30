// Control plane — Domains. Every domain registered on the deployment, which
// tenant owns it, and whether it is DNS-verified. Register a domain to a
// tenant, verify it (live DNS TXT check), and remove it. A verified domain is
// what the mail services require before a tenant may assign addresses in it.
import { useCallback, useEffect, useState } from "react";
import { Plus, Trash2 } from "lucide-react";

import { strings } from "../i18n";
import { Spinner, cx } from "../ds";
import { useJmapClient } from "../jmap";
import type { ControlDomain, ControlTenant } from "../jmap";
import { RegisterDomainModal } from "./RegisterDomainModal";
import styles from "../admin/admin.module.css";

export function DomainsPage() {
  const client = useJmapClient();
  const [domains, setDomains] = useState<ControlDomain[] | null>(null);
  const [tenants, setTenants] = useState<ControlTenant[]>([]);
  const [error, setError] = useState(false);
  const [adding, setAdding] = useState(false);
  const [note, setNote] = useState<string | null>(null);

  const load = useCallback(() => {
    setError(false);
    Promise.all([client.listDomains(), client.listTenants()])
      .then(([d, t]) => {
        setDomains(d);
        setTenants(t);
      })
      .catch(() => setError(true));
  }, [client]);

  useEffect(load, [load]);

  const tenantName = (id: string) => tenants.find((t) => t.id === id)?.name ?? id;

  async function verify(d: ControlDomain) {
    setNote(null);
    try {
      const res = await client.verifyDomain(d.domain);
      setNote(res.verified ? strings.domainVerifiedOk(d.domain) : (res.detail ?? strings.domainVerifyPending(d.domain)));
    } catch {
      setNote(strings.domainActionError);
    } finally {
      load();
    }
  }

  async function remove(d: ControlDomain) {
    if (!window.confirm(strings.domainDeleteConfirm(d.domain))) return;
    try {
      await client.deleteDomain(d.domain);
    } finally {
      load();
    }
  }

  return (
    <div className={styles.page}>
      <header className={styles.pageHead}>
        <div>
          <h1>{strings.controlDomains}</h1>
          <p className={styles.pageIntro}>{strings.controlDomainsIntro}</p>
        </div>
        <button
          type="button"
          className={styles.primary}
          onClick={() => setAdding(true)}
          disabled={tenants.length === 0}
        >
          <Plus size={16} />
          <span>{strings.domainAdd}</span>
        </button>
      </header>

      {note !== null && <p className={styles.checksFor}>{note}</p>}

      {domains === null && !error && (
        <div className={styles.state}>
          <Spinner size={22} />
        </div>
      )}
      {error && (
        <div className={styles.state}>
          <p>{strings.controlDomainsError}</p>
          <button type="button" className={styles.textBtn} onClick={load}>
            {strings.mailRetry}
          </button>
        </div>
      )}

      {domains !== null && domains.length === 0 && (
        <div className={styles.state}>
          <p>{strings.controlDomainsEmpty}</p>
        </div>
      )}

      {domains !== null && domains.length > 0 && (
        <ul className={styles.userList}>
          {domains.map((d) => (
            <li key={d.domain} className={styles.userRow}>
              <div className={styles.userText}>
                <div className={styles.userName}>
                  <strong>{d.domain}</strong>
                  <span
                    className={cx(styles.checkBadge, d.verified ? styles.chkPass : styles.chkWarn)}
                  >
                    {d.verified ? strings.domainVerified : strings.domainUnverified}
                  </span>
                </div>
                <div className={styles.userMeta}>{strings.domainOwnedBy(tenantName(d.tenantId))}</div>
              </div>
              <div className={styles.userActions}>
                {!d.verified && (
                  <button type="button" className={styles.ghost} onClick={() => void verify(d)}>
                    {strings.domainVerify}
                  </button>
                )}
                <button
                  type="button"
                  className={styles.iconBtn}
                  onClick={() => void remove(d)}
                  aria-label={strings.domainDelete}
                >
                  <Trash2 size={16} />
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}

      {adding && (
        <RegisterDomainModal
          tenants={tenants}
          onClose={() => setAdding(false)}
          onRegistered={() => {
            setAdding(false);
            load();
          }}
        />
      )}
    </div>
  );
}
