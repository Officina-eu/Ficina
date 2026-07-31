// The subset of JMAP (RFC 8620 core, RFC 8621 mail) the web app uses. These
// mirror the wire shapes `alo-jmap` returns; they are the client-side half
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
  /** alo extension: whether AI features are enabled for this tenant. */
  "alo:aiEnabled"?: boolean;
  /** alo extension: whether the signed-in user is a tenant admin. */
  "alo:isAdmin"?: boolean;
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
  /** Optional "#rrggbb" label color, or null. */
  color: string | null;
  parentId: string | null;
  sortOrder: number;
  totalEmails: number;
  unreadEmails: number;
}

/** A server-side mail filter (rule). Mirrors the alo-jmap rule model; the
 * server compiles these to a Sieve script that runs at delivery. */
export type FilterField = "from" | "to" | "cc" | "subject";
export type FilterOp = "contains" | "is";
export type FilterMatch = "all" | "any";

export interface FilterCondition {
  field: FilterField;
  op: FilterOp;
  value: string;
}

export type FilterAction =
  | { type: "fileInto"; mailbox: string }
  | { type: "markRead" }
  | { type: "star" }
  | { type: "delete" };

export interface MailFilterRule {
  id: string;
  name: string;
  match: FilterMatch;
  conditions: FilterCondition[];
  actions: FilterAction[];
  enabled: boolean;
}

export interface EmailHeaders {
  id: string;
  threadId: string;
  /** The raw RFC 822 message blob (for "show original", .eml, forward-as-attachment). */
  blobId: string;
  mailboxIds: Record<string, boolean>;
  keywords: Record<string, boolean>;
  from: EmailAddress[] | null;
  to: EmailAddress[] | null;
  cc: EmailAddress[] | null;
  /**
   * Blind-carbon recipients. Populated only on the sender's own (Sent/draft)
   * copy; a received copy always has this empty, so it never discloses another
   * recipient's blind copies.
   */
  bcc: EmailAddress[] | null;
  subject: string | null;
  receivedAt: string;
  size: number;
  preview: string;
  hasAttachment: boolean;
  /** RFC 5322 Message-ID(s), for reply threading. */
  messageId: string[] | null;
  references: string[] | null;
  /**
   * alo's parsed inbound-authentication verdict (non-standard, additive).
   * Absent on outgoing copies; each field is "pass" | "fail" | "none" | etc.
   */
  "alo:authentication"?: MessageAuthentication | null;
}

export interface MessageAuthentication {
  spf: string | null;
  dkim: string | null;
  dmarc: string | null;
}

export interface EmailBodyValue {
  value: string;
  isTruncated: boolean;
}

export interface EmailFull extends EmailHeaders {
  textBody: EmailBodyPart[];
  htmlBody: EmailBodyPart[];
  bodyValues: Record<string, EmailBodyValue>;
  attachments: EmailAttachment[];
}

export interface EmailBodyPart {
  partId: string | null;
  type: string;
}

/** A downloadable attachment on a message (JMAP EmailBodyPart, disposition
 * "attachment"). `blobId` resolves via the session download URL. */
export interface EmailAttachment {
  blobId: string;
  type: string;
  name: string;
  size: number;
}

/** One configured AI provider (admin console). The API key is never returned —
 * only whether one is set (`hasKey`). */
export interface AiProvider {
  id: string;
  kind: string;
  label: string;
  baseUrl: string;
  model: string;
  enabled: boolean;
  isDefault: boolean;
  hasKey: boolean;
}

/** A user in the admin console: identity + read-only usage + aliases. */
export interface AdminUser {
  id: string;
  email: string;
  isAdmin: boolean;
  createdAt: string;
  messageCount: number;
  storageBytes: number;
  aliases: string[];
}

/** A group in the admin console. `address` present means it's a distribution
 * list (mail to it fans out to members). */
export interface AdminGroup {
  id: string;
  name: string;
  address: string | null;
  memberCount: number;
  members: { id: string; email: string }[];
}

/** One deliverability check result (admin Security & trust). */
export interface SecurityCheck {
  key: string;
  title: string;
  status: "pass" | "warn" | "fail";
  detail: string;
}

/** One audit-log entry for the admin audit view (ADR 0012). `actor` is the
 * acting user's email, or a label like "operator", or null. */
export interface AuditEntry {
  id: string;
  actor: string | null;
  action: string;
  target: string | null;
  detail: string | null;
  at: string;
}

/** A tenant summary in the platform control plane (ADR 0012). */
export interface ControlTenant {
  id: string;
  name: string;
  status: "active" | "suspended";
  createdAt: string;
  userCount: number;
  storageBytes: number;
  /** Storage cap in bytes, or null for unlimited. */
  storageQuotaBytes: number | null;
}

/** A domain owned by a tenant (control plane). `verifyRecord` is the DNS TXT
 * record to publish to prove ownership. */
export interface ControlDomain {
  domain: string;
  tenantId: string;
  verified: boolean;
  verifiedAt: string | null;
  verifyRecord: { name: string; type: string; value: string };
  createdAt: string;
  /** The active DKIM record to publish (ADR 0014), present once verified; null
   * if no key yet. Only the tenant-admin `/admin/domains` listing includes it. */
  dkim?: { name: string; type: string; value: string; selector: string } | null;
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
