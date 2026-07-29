// The subset of JMAP (RFC 8620 core, RFC 8621 mail) the web app uses. These
// mirror the wire shapes `ficina-jmap` returns; they are the client-side half
// of the contract and change additively with it.

export const MAIL_CAPABILITY = "urn:ietf:params:jmap:mail";
export const CORE_CAPABILITY = "urn:ietf:params:jmap:core";
export const SUBMISSION_CAPABILITY = "urn:ietf:params:jmap:submission";

export interface Session {
  apiUrl: string;
  downloadUrl: string;
  uploadUrl: string;
  eventSourceUrl: string;
  /** capability URN → account id */
  primaryAccounts: Record<string, string>;
  state: string;
}

export interface EmailAddress {
  name: string | null;
  email: string;
}

export interface Mailbox {
  id: string;
  name: string;
  /** JMAP role: "inbox" | "sent" | "drafts" | "trash" | "archive" | "junk" | null */
  role: string | null;
  parentId: string | null;
  sortOrder: number;
  totalEmails: number;
  unreadEmails: number;
}

export interface EmailHeaders {
  id: string;
  threadId: string;
  mailboxIds: Record<string, boolean>;
  keywords: Record<string, boolean>;
  from: EmailAddress[] | null;
  to: EmailAddress[] | null;
  cc: EmailAddress[] | null;
  subject: string | null;
  receivedAt: string;
  size: number;
  preview: string;
  hasAttachment: boolean;
  /** RFC 5322 Message-ID(s), for reply threading. */
  messageId: string[] | null;
  references: string[] | null;
}

export interface EmailBodyValue {
  value: string;
  isTruncated: boolean;
}

export interface EmailFull extends EmailHeaders {
  textBody: EmailBodyPart[];
  htmlBody: EmailBodyPart[];
  bodyValues: Record<string, EmailBodyValue>;
}

export interface EmailBodyPart {
  partId: string | null;
  type: string;
}

/** A JMAP method invocation: [name, arguments, call-id]. */
export type MethodCall = [string, Record<string, unknown>, string];
export type MethodResponse = [string, Record<string, unknown>, string];

export interface JmapRequest {
  using: string[];
  methodCalls: MethodCall[];
}

export interface JmapResponse {
  methodResponses: MethodResponse[];
  sessionState: string;
}

/** JMAP keyword constants we read/set. */
export const KEYWORD_SEEN = "$seen";
export const KEYWORD_FLAGGED = "$flagged";
