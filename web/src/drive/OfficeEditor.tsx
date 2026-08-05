// The office compatibility editor (ADR 0010/0030): a real Word/Excel/PowerPoint
// file (.docx/.xlsx/.pptx/.odt/…) opened in Collabora, embedded same-origin. We
// mint a WOPI token, read Collabora's same-origin discovery to find the editor
// URL, and frame it; Collabora loads and saves the bytes via our /wopi host.
import { useEffect, useState } from "react";
import { X } from "lucide-react";

import { strings } from "../i18n";
import { useJmapClient } from "../jmap";
import { OFFICE_HOST } from "../platform/runtime";
import { Spinner } from "../ds";
import styles from "./OfficeEditor.module.css";

/** File name extensions that open in Collabora. */
export const OFFICE_EXT = /\.(docx?|xlsx?|pptx?|odt|ods|odp|rtf|csv)$/i;

export function OfficeEditor({
  nodeId,
  name,
  onClose,
}: {
  nodeId: string;
  name: string;
  onClose: () => void;
}) {
  const client = useJmapClient();
  const [src, setSrc] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let live = true;
    void (async () => {
      try {
        const token = await client.driveOfficeToken(nodeId);
        // Discovery is read same-origin (proxied in dev) to avoid CORS.
        const discovery = await (await fetch(`${window.location.origin}/hosting/discovery`)).text();
        const match = discovery.match(/\/browser\/[^"'?]+?\/cool\.html/);
        if (!match) throw new Error("no editor url");
        // The frame is loaded same-origin (proxied in dev, so no cross-origin
        // framing). But the WOPI file Collabora fetches server-side must be on a
        // host it can reach: same-origin in prod, the real backend in local dev
        // (Collabora can't reach the developer's localhost).
        const wopiSrc = `${OFFICE_HOST}/wopi/files/${encodeURIComponent(nodeId)}`;
        const url = `${window.location.origin}${match[0]}?WOPISrc=${encodeURIComponent(wopiSrc)}&access_token=${encodeURIComponent(token)}&lang=en`;
        if (live) setSrc(url);
      } catch {
        if (live) setFailed(true);
      }
    })();
    return () => {
      live = false;
    };
  }, [client, nodeId]);

  return (
    <div className={styles.overlay}>
      <header className={styles.head}>
        <button type="button" className={styles.back} onClick={onClose} aria-label={strings.close}>
          <X size={18} />
        </button>
        <span className={styles.name}>{name}</span>
      </header>
      <div className={styles.body}>
        {failed ? (
          <div className={styles.center}>{strings.officeUnavailable}</div>
        ) : src === null ? (
          <div className={styles.center}>
            <Spinner size={22} />
          </div>
        ) : (
          <iframe
            title={name}
            src={src}
            className={styles.frame}
            allow="clipboard-read; clipboard-write; fullscreen"
          />
        )}
      </div>
    </div>
  );
}
