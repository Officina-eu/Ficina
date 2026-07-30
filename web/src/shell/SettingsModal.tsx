// Account settings, opened from the account menu: the user's mail signature,
// and (for admins) the tenant-wide organization footer. Both are HTML the
// compose surface inserts into outgoing mail. Reuses the mail rich-text editor
// and the admin console's modal styles.
import { useEffect, useState } from "react";
import { X } from "lucide-react";

import { strings } from "../i18n";
import { Button, Spinner } from "../ds";
import { useJmapClient } from "../jmap";
import { RichTextEditor } from "../mail/components/RichTextEditor";
import styles from "../admin/admin.module.css";

interface SettingsModalProps {
  isAdmin: boolean;
  onClose: () => void;
}

export function SettingsModal({ isAdmin, onClose }: SettingsModalProps) {
  const client = useJmapClient();
  const [loaded, setLoaded] = useState(false);
  const [signature, setSignature] = useState("");
  const [orgFooter, setOrgFooter] = useState("");
  const [oooEnabled, setOooEnabled] = useState(false);
  const [oooSubject, setOooSubject] = useState("");
  const [oooMessage, setOooMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    client
      .mailSettings()
      .then((s) => {
        if (!live) return;
        setSignature(s.signature);
        setOrgFooter(s.orgFooter);
        setOooEnabled(s.outOfOffice.enabled);
        setOooSubject(s.outOfOffice.subject);
        setOooMessage(s.outOfOffice.message);
        setLoaded(true);
      })
      .catch(() => {
        if (live) setError(strings.settingsLoadError);
      });
    return () => {
      live = false;
    };
  }, [client]);

  async function save() {
    if (oooEnabled && oooMessage.trim() === "") {
      setError(strings.settingsOooNeedsMessage);
      return;
    }
    setBusy(true);
    setError(null);
    setNote(null);
    try {
      await client.setSignature(signature);
      await client.setOutOfOffice(oooEnabled, oooSubject, oooMessage);
      if (isAdmin) await client.setOrgFooter(orgFooter);
      setNote(strings.settingsSaved);
    } catch {
      setError(strings.settingsSaveError);
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
        aria-label={strings.settingsTitle}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className={styles.modalHead}>
          <h2>{strings.settingsTitle}</h2>
          <button
            type="button"
            className={styles.iconBtn}
            onClick={onClose}
            aria-label={strings.userClose}
          >
            <X size={18} />
          </button>
        </div>
        <div className={styles.modalBody}>
          {!loaded && error === null ? (
            <div className={styles.state}>
              <Spinner size={22} />
            </div>
          ) : (
            <>
              <div className={styles.field}>
                <span className={styles.label}>{strings.settingsSignature}</span>
                <div className={styles.sigEditor}>
                  <RichTextEditor
                    initialHtml={signature}
                    onChange={setSignature}
                    placeholder={strings.settingsSignatureHint}
                  />
                </div>
              </div>

              <div className={styles.field}>
                <label className={styles.oooToggleRow}>
                  <span className={styles.label}>{strings.settingsOutOfOffice}</span>
                  <span className={styles.toggle}>
                    <input
                      type="checkbox"
                      checked={oooEnabled}
                      onChange={(e) => setOooEnabled(e.target.checked)}
                    />
                    <span className={styles.track} />
                  </span>
                </label>
                <p className={styles.pageIntro}>{strings.settingsOutOfOfficeHint}</p>
                {oooEnabled && (
                  <>
                    <input
                      className={styles.input}
                      value={oooSubject}
                      onChange={(e) => setOooSubject(e.target.value)}
                      placeholder={strings.settingsOooSubjectPlaceholder}
                    />
                    <textarea
                      className={styles.textarea}
                      rows={4}
                      value={oooMessage}
                      onChange={(e) => setOooMessage(e.target.value)}
                      placeholder={strings.settingsOooMessagePlaceholder}
                    />
                  </>
                )}
              </div>
              {isAdmin && (
                <div className={styles.field}>
                  <span className={styles.label}>{strings.settingsOrgFooter}</span>
                  <p className={styles.pageIntro}>{strings.settingsOrgFooterHint}</p>
                  <div className={styles.sigEditor}>
                    <RichTextEditor
                      initialHtml={orgFooter}
                      onChange={setOrgFooter}
                      placeholder={strings.settingsOrgFooterPlaceholder}
                    />
                  </div>
                </div>
              )}
              {note !== null && <span className={styles.hintOk}>{note}</span>}
              {error !== null && (
                <p className={styles.error} role="alert">
                  {error}
                </p>
              )}
            </>
          )}
        </div>
        <div className={styles.modalFoot}>
          <div className={styles.footSpacer} />
          <button type="button" className={styles.textBtn} onClick={onClose}>
            {strings.userClose}
          </button>
          <Button onClick={() => void save()} disabled={busy || !loaded}>
            {busy ? <Spinner size={16} /> : strings.settingsSave}
          </Button>
        </div>
      </div>
    </div>
  );
}
