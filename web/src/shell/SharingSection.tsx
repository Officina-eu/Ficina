// Self-service mailbox sharing (ADR 0017) — the Gmail-style "grant access to
// your account" surface inside Settings. A user lists who can access their own
// mailbox, adds a colleague by email with an access level (read-only / manage)
// and send mode (none / send-as / send-on-behalf), or removes access. No admin
// needed; the server always treats the caller as the owner.
import { useCallback, useEffect, useState } from "react";
import type { FormEvent } from "react";
import { X } from "lucide-react";

import { strings } from "../i18n";
import { Spinner } from "../ds";
import { useJmapClient } from "../jmap";
import type { Delegate, SendMode } from "../jmap";
import styles from "./SharingSection.module.css";

export function SharingSection() {
  const client = useJmapClient();
  const [delegates, setDelegates] = useState<Delegate[] | null>(null);
  const [email, setEmail] = useState("");
  const [canWrite, setCanWrite] = useState(true);
  const [sendMode, setSendMode] = useState<SendMode>("none");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    setError(null);
    void client
      .myDelegates()
      .then(setDelegates)
      .catch(() => setError(strings.delegateError));
  }, [client]);
  useEffect(load, [load]);

  async function add(e: FormEvent) {
    e.preventDefault();
    if (email.trim().length === 0 || busy) return;
    setBusy(true);
    setError(null);
    try {
      await client.shareMyMailbox(email.trim(), canWrite, sendMode);
      setEmail("");
      setCanWrite(true);
      setSendMode("none");
      load();
    } catch {
      setError(strings.sharingAddError);
    } finally {
      setBusy(false);
    }
  }

  async function update(d: Delegate, nextWrite: boolean, nextSend: SendMode) {
    setBusy(true);
    try {
      await client.shareMyMailbox(d.email, nextWrite, nextSend);
      load();
    } catch {
      setError(strings.delegateError);
    } finally {
      setBusy(false);
    }
  }

  async function remove(id: string) {
    setBusy(true);
    try {
      await client.unshareMyMailbox(id);
      load();
    } catch {
      setError(strings.delegateError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className={styles.wrap}>
      {delegates === null ? (
        <Spinner size={18} />
      ) : delegates.length === 0 ? (
        <p className={styles.none}>{strings.sharingNone}</p>
      ) : (
        <ul className={styles.list}>
          {delegates.map((d) => (
            <li key={d.id} className={styles.row}>
              <span className={styles.email}>{d.email}</span>
              <Selects
                canWrite={d.canWrite}
                sendMode={d.sendMode}
                disabled={busy}
                onChange={(w, s) => void update(d, w, s)}
              />
              <button
                type="button"
                className={styles.remove}
                onClick={() => void remove(d.id)}
                aria-label={strings.delegateRemove}
              >
                <X size={16} />
              </button>
            </li>
          ))}
        </ul>
      )}

      <form className={styles.addRow} onSubmit={add}>
        <input
          className={styles.input}
          type="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          placeholder={strings.sharingEmailPlaceholder}
          aria-label={strings.sharingEmailPlaceholder}
        />
        <Selects
          canWrite={canWrite}
          sendMode={sendMode}
          disabled={busy}
          onChange={(w, s) => {
            setCanWrite(w);
            setSendMode(s);
          }}
        />
        <button type="submit" className={styles.add} disabled={busy || email.trim().length === 0}>
          {strings.sharingAdd}
        </button>
      </form>

      {error !== null && (
        <p className={styles.error} role="alert">
          {error}
        </p>
      )}
    </div>
  );
}

/** Access-level + send-mode selects. Sending implies manage. */
function Selects({
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
    <span className={styles.selects}>
      <select
        className={styles.select}
        value={sendMode === "none" && !canWrite ? "read" : "manage"}
        disabled={disabled || sendMode !== "none"}
        onChange={(e) => onChange(e.target.value === "manage", sendMode)}
        aria-label={strings.delegateAccessLabel}
      >
        <option value="read">{strings.delegateReadOnly}</option>
        <option value="manage">{strings.delegateManage}</option>
      </select>
      <select
        className={styles.select}
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
