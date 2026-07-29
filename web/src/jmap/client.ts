// A small typed JMAP client. It is constructed with the auth layer's
// `authorizedFetch` (bearer + refresh handled there), fetches and caches the
// session, and exposes the handful of mail calls the UI needs. Errors are
// normalized to a single JmapError the UI can render.
import {
  CORE_CAPABILITY,
  MAIL_CAPABILITY,
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
  "subject",
  "receivedAt",
  "size",
  "preview",
  "hasAttachment",
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
      using: [CORE_CAPABILITY, MAIL_CAPABILITY],
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

  /** One message with its body, for the reading pane. */
  async email(id: string): Promise<EmailFull | null> {
    const accountId = await this.accountId();
    const res = await this.#request([
      [
        "Email/get",
        {
          accountId,
          ids: [id],
          properties: [...HEADER_PROPS, "textBody", "htmlBody", "bodyValues"],
          fetchTextBodyValues: true,
          fetchHTMLBodyValues: true,
        },
        "e",
      ],
    ]);
    const list = this.#result(res, "e").list as EmailFull[];
    return list[0] ?? null;
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
}
