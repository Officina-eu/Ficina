// Pure helpers to pull display text out of a JMAP Email. Kept separate from
// the component so body selection is testable and the rendering component
// stays about safety and layout.
import type { EmailBodyPart, EmailFull } from "../jmap";

function join(parts: EmailBodyPart[], values: EmailFull["bodyValues"]): string | null {
  const chunks: string[] = [];
  for (const part of parts) {
    if (part.partId === null) continue;
    const v = values[part.partId];
    if (v !== undefined) chunks.push(v.value);
  }
  return chunks.length > 0 ? chunks.join("\n") : null;
}

/** The plain-text body, if the message has one. */
export function textContent(email: EmailFull): string | null {
  return join(email.textBody, email.bodyValues);
}

/** The HTML body, if the message has one. */
export function htmlContent(email: EmailFull): string | null {
  return join(email.htmlBody, email.bodyValues);
}

/** A rough plain-text rendering of a message body for feeding to the summarizer
 * (never for display): prefer the text part, else strip tags off the HTML. */
function plainBody(email: EmailFull): string {
  const text = textContent(email);
  if (text !== null) return text;
  const html = htmlContent(email);
  if (html === null) return email.preview;
  return html
    .replace(/<(script|style)[\s\S]*?<\/\1>/gi, " ")
    .replace(/<[^>]+>/g, " ")
    .replace(/&nbsp;/gi, " ")
    .replace(/\s+/g, " ")
    .trim();
}

/** Concatenate a thread into a labelled plain-text digest for summarization.
 * Order is the display order (oldest first); each turn is prefixed with its
 * sender so the model can attribute statements. */
export function threadDigest(messages: EmailFull[]): string {
  return messages
    .map((m) => {
      const who = m.from?.[0]?.name ?? m.from?.[0]?.email ?? "Unknown";
      return `${who}:\n${plainBody(m)}`;
    })
    .join("\n\n---\n\n")
    .trim();
}

/** Wrap untrusted HTML with a strict CSP for a sandboxed iframe: no scripts,
 * no remote anything (blocks tracking pixels — privacy is the brand); inline
 * styles and data: images (inline attachments) are allowed. */
export function sandboxedHtml(html: string): string {
  const csp =
    "default-src 'none'; img-src data:; style-src 'unsafe-inline'; font-src data:; media-src data:";
  return `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${csp}"><style>body{font-family:Georgia,serif;color:#211d18;margin:0;padding:16px;line-height:1.6}img{max-width:100%}</style></head><body>${html}</body></html>`;
}
