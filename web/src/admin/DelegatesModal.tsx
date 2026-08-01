// Admin — manage who can access a user's mailbox (ADR 0017 delegation). Each
// delegate has an access level (read-only vs manage) and a send mode (none /
// send-as / send-on-behalf), editable inline; an admin can add a user or revoke
// access. All writes go through the admin-gated /admin/delegates routes.
import { useCallback, useEffect, useState } from "react";
import { X } from "lucide-react";

import { strings } from "../i18n";
import { Spinner } from "../ds";
import { useJmapClient } from "../jmap";
import type { AdminUser, Delegate, SendMode } from "../jmap";
import styles from "./admin.module.css";

interface DelegatesModalProps {
  owner: AdminUser;
  users: AdminUser[];
  onClose: () => void;
}

export function DelegatesModal({ owner, users, onClose }: DelegatesModalProps) {
  const client = useJmapClient();
  const [delegates, setDelegates] = useState<Delegate[] | null>(null);
  const [pick, setPick] = useState("");
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

  async function grant(delegateId: string, canWrite: boolean, sendMode: SendMode) {
    setBusy(true);
    setError(null);
    try {
      await client.grantDelegate(owner.id, delegateId, canWrite, sendMode);
      setPick("");
      load();
    } catch {
      setError(strings.delegateError);
    } finally {
      setBusy(false);
    }
  }

  async function revoke(delegateId: string) {
    setBusy(true);
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

          {delegates === null ? (
            <Spinner size={18} />
          ) : delegates.length === 0 ? (
            <p className={styles.hint}>{strings.delegateNone}</p>
          ) : (
            <ul className={styles.delegateList}>
              {delegates.map((d) => (
                <li key={d.id} className={styles.delegateRow}>
                  <span className={styles.delegateEmail}>{d.email}</span>
                  <AccessControls
                    canWrite={d.canWrite}
                    sendMode={d.sendMode}
                    disabled={busy}
                    onChange={(w, s) => void grant(d.id, w, s)}
                  />
                  <button
                    type="button"
                    className={styles.iconBtn}
                    onClick={() => void revoke(d.id)}
                    aria-label={strings.delegateRemove}
                  >
                    <X size={16} />
                  </button>
                </li>
              ))}
            </ul>
          )}

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
            <button
              type="button"
              className={styles.ghost}
              onClick={() => void grant(pick, true, "none")}
              disabled={busy || pick.length === 0}
            >
              {strings.delegateAdd}
            </button>
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

/** The access-level + send-mode selects for one delegate. Sending implies
 * manage, so choosing a send mode also upgrades access. */
export function AccessControls({
  canWrite,
  sendMode,
  disabled,
  onChange,
}: {
  canWrite: boolean;
  sendMode: SendMode;
  disabled: boolean;
  onChange: (canWrite: boolean, sendMode: SendMode) => void;
}) {
  return (
    <span className={styles.accessControls}>
      <select
        className={styles.accessSelect}
        value={sendMode === "none" && !canWrite ? "read" : "manage"}
        disabled={disabled || sendMode !== "none"}
        onChange={(e) => onChange(e.target.value === "manage", sendMode)}
        aria-label={strings.delegateAccessLabel}
      >
        <option value="read">{strings.delegateReadOnly}</option>
        <option value="manage">{strings.delegateManage}</option>
      </select>
      <select
        className={styles.accessSelect}
        value={sendMode}
        disabled={disabled}
        onChange={(e) => {
          const s = e.target.value as SendMode;
          onChange(s === "none" ? canWrite : true, s);
        }}
        aria-label={strings.delegateSendLabel}
      >
        <option value="none">{strings.delegateSendNone}</option>
        <option value="as">{strings.delegateSendAs}</option>
        <option value="on_behalf">{strings.delegateSendOnBehalf}</option>
      </select>
    </span>
  );
}
