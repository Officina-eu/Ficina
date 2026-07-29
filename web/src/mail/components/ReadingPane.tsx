// The reading pane. Header in Inter (sender, recipients, date), body in
// Garamond for plain text; HTML-only bodies render in a sandboxed, CSP-locked
// iframe so untrusted markup can neither run scripts nor load remote content.
import { strings } from "../../i18n";
import { Spinner } from "../../ds";
import { Avatar } from "../../ds";
import type { EmailAddress, EmailFull } from "../../jmap";
import type { Async } from "../state/useAsync";
import { formatDate, senderName, subjectOr } from "../format";
import { htmlContent, sandboxedHtml, textContent } from "../body";
import styles from "./ReadingPane.module.css";

function addressLine(addresses: EmailAddress[] | null): string {
  if (addresses === null || addresses.length === 0) return "";
  return addresses.map((a) => (a.name !== null && a.name.length > 0 ? a.name : a.email)).join(", ");
}

interface ReadingPaneProps {
  email: Async<EmailFull | null>;
}

export function ReadingPane({ email }: ReadingPaneProps) {
  if (email.status === "loading") {
    return (
      <div className={styles.state}>
        <Spinner size={24} />
      </div>
    );
  }
  if (email.status === "error") {
    return (
      <div className={styles.state}>
        <p>{strings.mailListError}</p>
        <button type="button" className={styles.retry} onClick={email.reload}>
          {strings.mailRetry}
        </button>
      </div>
    );
  }

  const message = email.data;
  if (message === null) {
    return (
      <div className={styles.state}>
        <p>{strings.mailSelectPrompt}</p>
      </div>
    );
  }

  const text = textContent(message);
  const html = text === null ? htmlContent(message) : null;

  return (
    <article className={styles.pane}>
      <header className={styles.header}>
        <h1 className={styles.subject}>{subjectOr(message)}</h1>
        <div className={styles.meta}>
          <Avatar name={senderName(message)} email={message.from?.[0]?.email} size="md" />
          <div className={styles.metaText}>
            <div className={styles.senderRow}>
              <span className={styles.sender}>{senderName(message)}</span>
              <span className={styles.date}>{formatDate(message.receivedAt)}</span>
            </div>
            <div className={styles.recipients}>
              {strings.mailTo}: {addressLine(message.to)}
            </div>
          </div>
        </div>
      </header>

      <div className={styles.bodyScroll}>
        {text !== null && <pre className={styles.text}>{text}</pre>}
        {html !== null && (
          <iframe
            className={styles.html}
            title={subjectOr(message)}
            sandbox=""
            srcDoc={sandboxedHtml(html)}
          />
        )}
        {text === null && html === null && <p className={styles.empty}>{message.preview}</p>}
      </div>
    </article>
  );
}
