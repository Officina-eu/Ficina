// A small typed JMAP client. It is constructed with the auth layer's
// `authorizedFetch` (bearer + refresh handled there), fetches and caches the
// session, and exposes the handful of mail calls the UI needs. Errors are
// normalized to a single JmapError the UI can render.
import {
  CORE_CAPABILITY,
  MAIL_CAPABILITY,
  SUBMISSION_CAPABILITY,
  type EmailAddress,
  type EmailFull,
  type EmailHeaders,
  type JmapRequest,
  type JmapResponse,
  type Mailbox,
  type MethodCall,
  type Session,
} from "./types";

export class JmapError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "JmapError";
  }
}

type AuthorizedFetch = (input: string, init?: RequestInit) => Promise<Response>;

const HEADER_PROPS = [
  "id",
  "threadId",
  "mailboxIds",
  "keywords",
  "from",
  "to",
  "cc",
  "subject",
  "receivedAt",
  "size",
  "preview",
  "hasAttachment",
  "messageId",
  "references",
];

export class JmapClient {
  #fetch: AuthorizedFetch;
  #session: Session | null = null;

  constructor(authorizedFetch: AuthorizedFetch) {
    this.#fetch = authorizedFetch;
  }

  /** Fetch (and cache) the JMAP session; yields the primary mail account id. */
  async session(): Promise<Session> {
    if (this.#session !== null) return this.#session;
    let response: Response;
    try {
      response = await this.#fetch("/.well-known/jmap");
    } catch (err) {
      throw new JmapError(err instanceof Error ? err.message : "network error");
    }
    if (!response.ok) throw new JmapError(`session ${response.status}`);
    const session = (await response.json()) as Session;
    this.#session = session;
    return session;
  }

  async accountId(): Promise<string> {
    const session = await this.session();
    const id = session.primaryAccounts[MAIL_CAPABILITY];
    if (id === undefined) throw new JmapError("no mail account");
    return id;
  }

  async #request(methodCalls: MethodCall[]): Promise<JmapResponse> {
    const session = await this.session();
    const body: JmapRequest = {
      using: [CORE_CAPABILITY, MAIL_CAPABILITY, SUBMISSION_CAPABILITY],
      methodCalls,
    };
    let response: Response;
    try {
      response = await this.#fetch(session.apiUrl, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body),
      });
    } catch (err) {
      throw new JmapError(err instanceof Error ? err.message : "network error");
    }
    if (!response.ok) throw new JmapError(`request ${response.status}`);
    return (await response.json()) as JmapResponse;
  }

  #result(res: JmapResponse, callId: string): Record<string, unknown> {
    const found = res.methodResponses.find((m) => m[2] === callId);
    if (found === undefined) throw new JmapError("missing method response");
    if (found[0] === "error") {
      const type = (found[1] as { type?: string }).type ?? "unknown";
      throw new JmapError(`JMAP error: ${type}`);
    }
    return found[1];
  }

  /** All of the account's mailboxes (folders). */
  async mailboxes(): Promise<Mailbox[]> {
    const accountId = await this.accountId();
    const res = await this.#request([["Mailbox/get", { accountId, ids: null }, "m"]]);
    return (this.#result(res, "m").list as Mailbox[]) ?? [];
  }

  /** Header rows for a mailbox, newest first, via query + back-referenced get. */
  async emailHeaders(mailboxId: string, limit = 60): Promise<EmailHeaders[]> {
    const accountId = await this.accountId();
    const res = await this.#request([
      [
        "Email/query",
        {
          accountId,
          filter: { inMailbox: mailboxId },
          sort: [{ property: "receivedAt", isAscending: false }],
          limit,
        },
        "q",
      ],
      [
        "Email/get",
        {
          accountId,
          "#ids": { resultOf: "q", name: "Email/query", path: "/ids" },
          properties: HEADER_PROPS,
        },
        "g",
      ],
    ]);
    return (this.#result(res, "g").list as EmailHeaders[]) ?? [];
  }

  /** All messages of a thread, with bodies, oldest-first (for the conversation
   * view). One request: Thread/get feeds Email/get by back-reference. */
  async threadEmails(threadId: string): Promise<EmailFull[]> {
    const accountId = await this.accountId();
    const res = await this.#request([
      ["Thread/get", { accountId, ids: [threadId] }, "t"],
      [
        "Email/get",
        {
          accountId,
          "#ids": { resultOf: "t", name: "Thread/get", path: "/list/0/emailIds" },
          properties: [...HEADER_PROPS, "textBody", "htmlBody", "bodyValues", "attachments"],
          fetchTextBodyValues: true,
          fetchHTMLBodyValues: true,
        },
        "e",
      ],
    ]);
    const list = (this.#result(res, "e").list as EmailFull[]) ?? [];
    return [...list].sort((a, b) => a.receivedAt.localeCompare(b.receivedAt));
  }

  /** One message with its body, for the reading pane. */
  async email(id: string): Promise<EmailFull | null> {
    const accountId = await this.accountId();
    const res = await this.#request([
      [
        "Email/get",
        {
          accountId,
          ids: [id],
          properties: [...HEADER_PROPS, "textBody", "htmlBody", "bodyValues", "attachments"],
          fetchTextBodyValues: true,
          fetchHTMLBodyValues: true,
        },
        "e",
      ],
    ]);
    const list = this.#result(res, "e").list as EmailFull[];
    return list[0] ?? null;
  }

  /** Fetch an attachment's bytes as a Blob (authorized), for saving. Resolves
   * the session download URL template with the account, blob id, and name. */
  async downloadAttachment(blobId: string, name: string): Promise<Blob> {
    const session = await this.session();
    const accountId = await this.accountId();
    const url = session.downloadUrl
      .replace("{accountId}", encodeURIComponent(accountId))
      .replace("{blobId}", encodeURIComponent(blobId))
      .replace("{name}", encodeURIComponent(name));
    const res = await this.#fetch(url, { method: "GET" });
    if (!res.ok) throw new JmapError(`download ${res.status}`);
    return res.blob();
  }

  /** Mark a message read/unread by toggling the $seen keyword. */
  async setSeen(id: string, seen: boolean): Promise<void> {
    await this.#setKeyword(id, "$seen", seen);
  }

  /** Flag/unflag a message ($flagged keyword). */
  async setFlagged(id: string, flagged: boolean): Promise<void> {
    await this.#setKeyword(id, "$flagged", flagged);
  }

  async #setKeyword(id: string, keyword: string, on: boolean): Promise<void> {
    const accountId = await this.accountId();
    const res = await this.#request([
      ["Email/set", { accountId, update: { [id]: { [`keywords/${keyword}`]: on ? true : null } } }, "s"],
    ]);
    this.#result(res, "s");
  }

  /** Move a message from one mailbox to another (e.g. archive). */
  async move(id: string, fromMailboxId: string, toMailboxId: string): Promise<void> {
    const accountId = await this.accountId();
    const res = await this.#request([
      [
        "Email/set",
        {
          accountId,
          update: { [id]: { [`mailboxIds/${fromMailboxId}`]: null, [`mailboxIds/${toMailboxId}`]: true } },
        },
        "m",
      ],
    ]);
    this.#result(res, "m");
  }

  /** Permanently delete a message. */
  async destroy(id: string): Promise<void> {
    const accountId = await this.accountId();
    const res = await this.#request([["Email/set", { accountId, destroy: [id] }, "d"]]);
    this.#result(res, "d");
  }

  /** Mark several messages read/unread in one call (whole-conversation). */
  async setSeenMany(ids: string[], seen: boolean): Promise<void> {
    if (ids.length === 0) return;
    const accountId = await this.accountId();
    const update: Record<string, unknown> = {};
    for (const id of ids) update[id] = { "keywords/$seen": seen ? true : null };
    const res = await this.#request([["Email/set", { accountId, update }, "s"]]);
    this.#result(res, "s");
  }

  /** Move several messages from one mailbox to another in one call. */
  async moveMany(ids: string[], fromMailboxId: string, toMailboxId: string): Promise<void> {
    if (ids.length === 0) return;
    const accountId = await this.accountId();
    const update: Record<string, unknown> = {};
    for (const id of ids) {
      update[id] = { [`mailboxIds/${fromMailboxId}`]: null, [`mailboxIds/${toMailboxId}`]: true };
    }
    const res = await this.#request([["Email/set", { accountId, update }, "m"]]);
    this.#result(res, "m");
  }

  /** Permanently delete several messages in one call. */
  async destroyMany(ids: string[]): Promise<void> {
    if (ids.length === 0) return;
    const accountId = await this.accountId();
    const res = await this.#request([["Email/set", { accountId, destroy: ids }, "d"]]);
    this.#result(res, "d");
  }

  /** Upload a file's bytes to the blob store; returns its blob id + type/size.
   * The blob is unreferenced until a draft embeds it (and is GC'd if never used). */
  async uploadFile(file: File): Promise<{ blobId: string; type: string; size: number }> {
    const session = await this.session();
    const accountId = await this.accountId();
    const url = session.uploadUrl.replace("{accountId}", encodeURIComponent(accountId));
    const res = await this.#fetch(url, {
      method: "POST",
      headers: { "content-type": file.type.length > 0 ? file.type : "application/octet-stream" },
      body: file,
    });
    if (!res.ok) throw new JmapError(`upload ${res.status}`);
    const json = (await res.json()) as { blobId: string; type: string; size: number };
    return { blobId: json.blobId, type: json.type, size: json.size };
  }

  /** Create a draft message; returns the new email id. */
  async createDraft(params: {
    mailboxId: string;
    from: EmailAddress;
    to: EmailAddress[];
    cc?: EmailAddress[];
    subject: string;
    bodyText: string;
    bodyHtml?: string;
    inReplyTo?: string[];
    references?: string[];
    attachments?: { blobId: string; type: string; name: string }[];
  }): Promise<string> {
    const accountId = await this.accountId();
    const bodyValues: Record<string, { value: string }> = { text: { value: params.bodyText } };
    const email: Record<string, unknown> = {
      mailboxIds: { [params.mailboxId]: true },
      keywords: { $draft: true },
      from: [params.from],
      to: params.to,
      subject: params.subject,
      bodyValues,
      textBody: [{ partId: "text", type: "text/plain" }],
    };
    if (params.bodyHtml !== undefined && params.bodyHtml.length > 0) {
      bodyValues.html = { value: params.bodyHtml };
      email.htmlBody = [{ partId: "html", type: "text/html" }];
    }
    if (params.attachments !== undefined && params.attachments.length > 0) {
      email.attachments = params.attachments.map((a) => ({
        blobId: a.blobId,
        type: a.type,
        name: a.name,
        disposition: "attachment",
      }));
    }
    if (params.cc !== undefined && params.cc.length > 0) email.cc = params.cc;
    if (params.inReplyTo !== undefined && params.inReplyTo.length > 0) email.inReplyTo = params.inReplyTo;
    if (params.references !== undefined && params.references.length > 0) email.references = params.references;
    const res = await this.#request([["Email/set", { accountId, create: { draft: email } }, "c"]]);
    const result = this.#result(res, "c");
    const created = (result.created as Record<string, { id: string }> | undefined)?.draft;
    if (created === undefined) {
      throw new JmapError("the draft could not be created");
    }
    return created.id;
  }

  /** Submit a draft for delivery; the server sends it and files it to Sent. */
  async submitEmail(emailId: string, mailFrom: string, rcptTo: string[]): Promise<void> {
    const accountId = await this.accountId();
    const res = await this.#request([
      [
        "EmailSubmission/set",
        {
          accountId,
          create: {
            sub: {
              emailId,
              envelope: {
                mailFrom: { email: mailFrom },
                rcptTo: rcptTo.map((email) => ({ email })),
              },
            },
          },
        },
        "s",
      ],
    ]);
    const result = this.#result(res, "s");
    const created = (result.created as Record<string, unknown> | undefined)?.sub;
    if (created === undefined) {
      const notCreated = (
        result.notCreated as Record<string, { description?: string; type?: string }> | undefined
      )?.sub;
      throw new JmapError(notCreated?.description ?? notCreated?.type ?? "the message could not be sent");
    }
  }
}
