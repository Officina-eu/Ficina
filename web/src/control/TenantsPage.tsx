// Control plane — Tenants. Lists every tenant on the deployment with usage and
// lifecycle status; provision a tenant, suspend/resume, and delete. Delete is
// guarded by an id-echo confirmation on the server; the UI confirms too.
import { useCallback, useEffect, useState } from "react";
import { Plus, Trash2 } from "lucide-react";

import { strings } from "../i18n";
import { Spinner, cx, useDialogs } from "../ds";
import { useJmapClient } from "../jmap";
import type { ControlTenant } from "../jmap";
import { formatBytes } from "../mail/format";
import { CreateTenantModal } from "./CreateTenantModal";
import styles from "../admin/admin.module.css";

export function TenantsPage() {
  const { confirm, prompt } = useDialogs();
  const client = useJmapClient();
  const [tenants, setTenants] = useState<ControlTenant[] | null>(null);
  const [error, setError] = useState(false);
  const [creating, setCreating] = useState(false);

  const load = useCallback(() => {
    setError(false);
    client
      .listTenants()
      .then(setTenants)
      .catch(() => setError(true));
  }, [client]);

  useEffect(load, [load]);

  async function toggleStatus(t: ControlTenant) {
    const next = t.status === "active" ? "suspended" : "active";
    setTenants((prev) => (prev ?? []).map((x) => (x.id === t.id ? { ...x, status: next } : x)));
    try {
      await client.setTenantStatus(t.id, next);
    } finally {
      load();
    }
  }

  async function remove(t: ControlTenant) {
    if (!(await confirm({ message: strings.tenantDeleteConfirm(t.name), danger: true }))) return;
    try {
      await client.deleteTenant(t.id, t.id);
    } finally {
      load();
    }
  }

  async function setQuota(t: ControlTenant) {
    const current = t.storageQuotaBytes === null ? "" : String(t.storageQuotaBytes / 1_000_000_000);
    const answer = await prompt({ message: strings.tenantQuotaPrompt, defaultValue: current });
    if (answer === null) return; // cancelled
    const trimmed = answer.trim();
    let quotaBytes: number | null;
    if (trimmed === "") {
      quotaBytes = null; // unlimited
    } else {
      const gb = Number(trimmed);
      if (!Number.isFinite(gb) || gb < 0) return;
      quotaBytes = Math.round(gb * 1_000_000_000);
    }
    try {
      await client.setTenantQuota(t.id, quotaBytes);
    } finally {
      load();
    }
  }

  return (
    <div className={styles.page}>
      <header className={styles.pageHead}>
        <div>
          <h1>{strings.controlTenants}</h1>
          <p className={styles.pageIntro}>{strings.controlTenantsIntro}</p>
        </div>
        <button type="button" className={styles.primary} onClick={() => setCreating(true)}>
          <Plus size={16} />
          <span>{strings.tenantAdd}</span>
        </button>
      </header>

      {tenants === null && !error && (
        <div className={styles.state}>
          <Spinner size={22} />
        </div>
      )}
      {error && (
        <div className={styles.state}>
          <p>{strings.controlTenantsError}</p>
          <button type="button" className={styles.textBtn} onClick={load}>
            {strings.mailRetry}
          </button>
        </div>
      )}

      {tenants !== null && tenants.length === 0 && (
        <div className={styles.state}>
          <p>{strings.controlTenantsEmpty}</p>
        </div>
      )}

      {tenants !== null && tenants.length > 0 && (
        <ul className={styles.userList}>
          {tenants.map((t) => (
            <li key={t.id} className={styles.userRow}>
              <div className={styles.userText}>
                <div className={styles.userName}>
                  <strong>{t.name}</strong>
                  <span
                    className={cx(
                      styles.checkBadge,
                      t.status === "active" ? styles.chkPass : styles.chkWarn,
                    )}
                  >
                    {t.status === "active" ? strings.tenantActive : strings.tenantSuspended}
                  </span>
                </div>
                <div className={styles.userMeta}>
                  {strings.tenantUsage(t.userCount, formatBytes(t.storageBytes))}
                  {" · "}
                  {t.storageQuotaBytes === null
                    ? strings.tenantQuotaUnlimited
                    : strings.tenantQuotaOf(formatBytes(t.storageQuotaBytes))}
                </div>
              </div>
              <div className={styles.userActions}>
                <button type="button" className={styles.ghost} onClick={() => void setQuota(t)}>
                  {strings.tenantQuota}
                </button>
                <button type="button" className={styles.ghost} onClick={() => void toggleStatus(t)}>
                  {t.status === "active" ? strings.tenantSuspend : strings.tenantResume}
                </button>
                <button
                  type="button"
                  className={styles.iconBtn}
                  onClick={() => void remove(t)}
                  aria-label={strings.tenantDelete}
                >
                  <Trash2 size={16} />
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}

      {creating && (
        <CreateTenantModal
          onClose={() => setCreating(false)}
          onCreated={() => {
            setCreating(false);
            load();
          }}
        />
      )}
    </div>
  );
}
