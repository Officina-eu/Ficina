// Mail data hooks over the JMAP client. One concern: turning client calls into
// loading/ready/error state the components render. Selection lives in the
// module component, not here.
import { useCallback } from "react";

import { useJmapClient } from "../../jmap";
import type { Category, EmailFull, EmailHeaders, Mailbox } from "../../jmap";
import { useAsync } from "./useAsync";
import type { Async } from "./useAsync";

/** All mailboxes (folders) for the account. */
export function useMailboxes(): Async<Mailbox[]> {
  const client = useJmapClient();
  return useAsync(useCallback(() => client.mailboxes(), [client]));
}

/** The account's categories (colored labels). */
export function useCategories(): Async<Category[]> {
  const client = useJmapClient();
  return useAsync(useCallback(() => client.categories(), [client]));
}

/** Header rows for a mailbox, optionally filtered to one category (null
 * selection yields an empty, ready list). */
export function useEmailHeaders(
  mailboxId: string | null,
  categoryId?: string | null,
): Async<EmailHeaders[]> {
  const client = useJmapClient();
  return useAsync(
    useCallback(
      () =>
        mailboxId === null
          ? Promise.resolve([])
          : client.emailHeaders(mailboxId, 60, categoryId ?? undefined),
      [client, mailboxId, categoryId],
    ),
  );
}

/** The cross-folder "Flagged" smart view (empty, ready list when inactive). */
export function useFlagged(active: boolean): Async<EmailHeaders[]> {
  const client = useJmapClient();
  return useAsync(
    useCallback(
      () => (active ? client.flaggedHeaders() : Promise.resolve([])),
      [client, active],
    ),
  );
}

/** One message with body (null selection yields null). */
export function useEmailBody(emailId: string | null): Async<EmailFull | null> {
  const client = useJmapClient();
  return useAsync(
    useCallback(
      () => (emailId === null ? Promise.resolve(null) : client.email(emailId)),
      [client, emailId],
    ),
  );
}

/** All messages of a thread, with bodies, oldest-first (null yields empty). */
export function useThread(threadId: string | null): Async<EmailFull[]> {
  const client = useJmapClient();
  return useAsync(
    useCallback(
      () => (threadId === null ? Promise.resolve([]) : client.threadEmails(threadId)),
      [client, threadId],
    ),
  );
}
