// Admin — manage who can access a user's mailbox (ADR 0017 delegation). Lists
// the delegates granted access to `owner`'s mailbox, lets an admin add a user
// (optionally with permission to send as the mailbox) or revoke access, and
// toggle the send permission per delegate. All writes go through the
// admin-gated /admin/delegates routes.
import { useCallback, useEffect, useState } from "react";
import { X } from "lucide-react";

import { strings } from "../i18n";
import { Spinner } from "../ds";
import { useJmapClient } from "../jmap";
import type { AdminUser } from "../jmap";
import styles from "./admin.module.css";

interface Delegate {
  id: string;
  email: string;
  canSend: boolean;
}

interface DelegatesModalProps {
  owner: AdminUser;
  users: AdminUser[];
  onClose: () => void;
}

export function DelegatesModal({ owner, users, onClose }: DelegatesModalProps) {
  const client = useJmapClient();
  const [delegates, setDelegates] = useState<Delegate[] | null>(null);
  const [pick, setPick] = useState("");
  const [canSend, setCanSend] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    setError(null);
    void client
      .listDelegates(owner.id)
      .then(setDelegates)
      .catch(() => setError(strings.delegateError));
  }, [client, owner.id]);
  useEffect(load, [load]);

  const addable = users.filter(
    (u) => u.id !== owner.id && !(delegates ?? []).some((d) => d.id === u.id),
  );

  async function grant(delegateId: string, send: boolean) {
    setBusy(true);
    setError(null);
    try {
      await client.grantDelegate(owner.id, delegateId, send);
      setPick("");
      setCanSend(false);
      load();
    } catch {
      setError(strings.delegateError);
    } finally {
      setBusy(false);
    }
  }

  async function revoke(delegateId: string) {
    setBusy(true);
    setError(null);
    try {
      await client.revokeDelegate(owner.id, delegateId);
      load();
    } catch {
      setError(strings.delegateError);
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
        aria-label={strings.delegateTitle(owner.email)}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className={styles.modalHead}>
          <h2>{strings.delegateTitle(owner.email)}</h2>
          <button type="button" className={styles.iconBtn} onClick={onClose} aria-label={strings.groupClose}>
            <X size={18} />
          </button>
        </div>
        <div className={styles.modalBody}>
          <p className={styles.hint}>{strings.delegateIntro}</p>
          <div className={styles.field}>
            <span className={styles.label}>{strings.delegatePeople}</span>
            <div className={styles.chips}>
              {delegates === null ? (
                <Spinner size={16} />
              ) : delegates.length === 0 ? (
                <span className={styles.hint}>{strings.delegateNone}</span>
              ) : (
                delegates.map((d) => (
                  <span key={d.id} className={styles.chip}>
                    <span className={styles.chipLabel}>{d.email}</span>
                    <button
                      type="button"
                      className={styles.ghost}
                      onClick={() => void grant(d.id, !d.canSend)}
                      disabled={busy}
                      title={strings.delegateSendToggle}
                    >
                      {d.canSend ? strings.delegateCanSend : strings.delegateReadOnly}
                    </button>
                    <button
                      type="button"
                      className={styles.chipX}
                      onClick={() => void revoke(d.id)}
                      aria-label={strings.delegateRemove}
                    >
                      <X size={12} />
                    </button>
                  </span>
                ))
              )}
            </div>
            <div className={styles.keyRow}>
              <select
                className={styles.input}
                value={pick}
                onChange={(e) => setPick(e.target.value)}
                disabled={addable.length === 0}
              >
                <option value="">{`${strings.delegateAdd}…`}</option>
                {addable.map((u) => (
                  <option key={u.id} value={u.id}>
                    {u.email}
                  </option>
                ))}
              </select>
              <label style={{ display: "inline-flex", alignItems: "center", gap: 6, whiteSpace: "nowrap" }}>
                <input type="checkbox" checked={canSend} onChange={(e) => setCanSend(e.target.checked)} />
                {strings.delegateAllowSend}
              </label>
              <button
                type="button"
                className={styles.ghost}
                onClick={() => void grant(pick, canSend)}
                disabled={busy || pick.length === 0}
              >
                {strings.delegateAdd}
              </button>
            </div>
          </div>
          {error !== null && (
            <p className={styles.error} role="alert">
              {error}
            </p>
          )}
        </div>
        <div className={styles.modalFoot}>
          <div className={styles.footSpacer} />
          <button type="button" className={styles.primary} onClick={onClose}>
            {strings.groupClose}
          </button>
        </div>
      </div>
    </div>
  );
}
