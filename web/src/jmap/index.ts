// Public surface of the shared JMAP area.
export { JmapClient, JmapError } from "./client";
export { useJmapClient } from "./useJmapClient";
export {
  MAIL_CAPABILITY,
  KEYWORD_SEEN,
  KEYWORD_FLAGGED,
  type Mailbox,
  type EmailHeaders,
  type EmailFull,
  type EmailBodyPart,
  type EmailBodyValue,
  type EmailAttachment,
  type EmailAddress,
  type MailFilterRule,
  type FilterCondition,
  type FilterAction,
  type FilterField,
  type FilterOp,
  type FilterMatch,
  type AiProvider,
  type AdminUser,
  type AdminGroup,
  type SecurityCheck,
  type AuditEntry,
  type ControlTenant,
  type ControlDomain,
  type Session,
} from "./types";
