// Provision a new tenant and its first admin (control plane). The operator
// sets the admin's email and initial password; the tenant, admin user, inbox,
// and login are created server-side in one call.
import { useState } from "react";
import type { FormEvent } from "react";
import { X } from "lucide-react";

import { strings } from "../i18n";
import { Button, Spinner } from "../ds";
import { useJmapClient } from "../jmap";
import styles from "../admin/admin.module.css";

interface Props {
  onClose: () => void;
  onCreated: () => void;
}

const MIN_ADMIN_PASSWORD = 12;

export function CreateTenantModal({ onClose, onCreated }: Props) {
  const client = useJmapClient();
  const [name, setName] = useState("");
  const [adminEmail, setAdminEmail] = useState("");
  const [adminPassword, setAdminPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (name.trim().length === 0 || !adminEmail.includes("@") || adminPassword.length < MIN_ADMIN_PASSWORD) {
      setError(strings.tenantInvalid);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await client.createTenant({
        name: name.trim(),
        adminEmail: adminEmail.trim(),
        adminPassword,
      });
      onCreated();
    } catch {
      setError(strings.tenantCreateError);
      setBusy(false);
    }
  }

  return (
    <div className={styles.overlay} onMouseDown={onClose}>
      <div
        className={styles.modal}
        role="dialog"
        aria-modal="true"
        aria-label={strings.tenantAdd}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <form onSubmit={submit}>
          <div className={styles.modalHead}>
            <h2>{strings.tenantAdd}</h2>
            <button type="button" className={styles.iconBtn} onClick={onClose} aria-label={strings.userClose}>
              <X size={18} />
            </button>
          </div>
          <div className={styles.modalBody}>
            <label className={styles.field}>
              <span className={styles.label}>{strings.tenantName}</span>
              <input
                className={styles.input}
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={strings.tenantNameHint}
                autoFocus
              />
            </label>
            <label className={styles.field}>
              <span className={styles.label}>{strings.tenantAdminEmail}</span>
              <input
                className={styles.input}
                value={adminEmail}
                onChange={(e) => setAdminEmail(e.target.value)}
                placeholder="admin@customer.example"
              />
            </label>
            <label className={styles.field}>
              <span className={styles.label}>{strings.tenantAdminPassword}</span>
              <input
                className={styles.input}
                type="password"
                value={adminPassword}
                onChange={(e) => setAdminPassword(e.target.value)}
                placeholder={strings.tenantAdminPasswordHint}
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
              {busy ? <Spinner size={16} /> : strings.tenantCreate}
            </Button>
          </div>
        </form>
      </div>
    </div>
  );
}
