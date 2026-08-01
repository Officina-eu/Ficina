# Shared mailboxes & delegation

*User guide. For the design and security model, see
[ADR 0017](../decisions/0017-send-as-and-mailbox-delegation.md).*

A **shared mailbox** is one mailbox that several people open and work together —
a team address like `info@aloworld.com` or `support@…`. **Delegation** is the
same mechanism pointed at a person's own inbox (e.g. an assistant managing a
manager's mail). Both are the same feature: a **grant** that lets one person
(the *delegate*) access another account (the *owner*).

alo matches how Outlook and Gmail work: one real mailbox, one copy of each
message, shared read state, and the option to send **as** the address or **on
behalf of** it.

---

## Key behaviour: read/unread is shared

A shared mailbox holds **one copy** of each message. When any member opens a new
email, it is marked **read for everyone**, and the mailbox's unread count drops
for all of them. This is what stops two people from replying to the same
message.

> Example: `info@aloworld.com` has five people with access. A new email arrives.
> One person opens it → the other four now see it as **read**.

This is different from a **distribution list** (`team@…`), which delivers a
**separate copy** into each member's own inbox — there, everyone has their own
independent read state. Use a distribution list for "send one address, everyone
gets their own copy"; use a shared mailbox for "one place we all work together".

This is **live**: alo consumes the server's push stream, so when one member
opens (or moves, flags, deletes) a message, the other connected members' views
update within a moment — no manual refresh needed. If a client is briefly
offline it catches up on reconnect. Newly **granting** someone is live too: the
shared mailbox appears in their sidebar and starts receiving updates the moment
access is given, without them reloading.

---

## Access levels

Each grant has an **access level**:

| Level | Can do | Cannot do |
|-------|--------|-----------|
| **Read-only** | Open, read, search the mailbox | Move, flag, delete, or send |
| **Can manage** | Read **and** move / flag / delete / organise | Send (unless also given a send permission) |

A read-only delegate that tries to change anything is refused by the server
(`accountReadOnly`) — read-only really is read-only.

### Limiting access to specific folders

By default a grant covers the **whole mailbox**. You can instead confine a
person to **specific folders** (Outlook-style per-folder delegation): in
**Settings → Sharing**, tick *Limit access to specific folders* when adding
someone, or use the folder button on an existing grant. They then see and touch
**only** those folders — every other folder is invisible to them (indistinguish­
able from not existing), they can't move a message into or out of an off-limits
folder, and they can't restructure the mailbox. Clearing the selection restores
whole-mailbox access.

Granting a folder **also grants its subfolders** — you don't have to tick each
child. Grant `Projects` and the delegate can work in `Projects / Q1`,
`Projects / Q2`, and anything nested below, automatically.

## Send permissions

On top of the access level, a grant may allow sending:

| Send permission | Result | Recipients see |
|-----------------|--------|----------------|
| **Can't send** (default) | View/manage only | — |
| **Send as** | The message goes out **as the shared address** | Just the shared address (e.g. `info@aloworld.com`) |
| **Send on behalf** | Same `From:`, plus a `Sender:` of the person who sent | "*Delegate* on behalf of *info@aloworld.com*" |

Choosing any send permission implies **manage** access (you can't send without
being able to create a draft). A delegate without a send permission who tries to
send is refused (`forbiddenToSend`).

---

## Setting it up

### Share your own mailbox (self-service — no admin)

1. Open **Settings → Sharing**.
2. Type the colleague's **email** (they must be in your organisation).
3. Pick an **access level** (Read-only / Can manage) and a **send permission**
   (Can't send / Send as / Send on behalf).
4. Click **Share**. Edit or remove access from the same list at any time.

### Set up a team mailbox (admin)

A team mailbox like `info@aloworld.com` is just a user account whose access is
shared with the team:

1. **Admin → Users** — create the mailbox as a user (`info@…`) if it doesn't
   exist yet.
2. On that user, click **Shared access**.
3. Add each team member, with the access level and send permission they should
   have. Any number of people can be added.

An admin can manage the delegates of **any** mailbox; a regular user can manage
only **their own**.

---

## Opening a shared mailbox

Every mailbox you can access is **mounted in the folder sidebar** at once
(Outlook-style): your own folders at the top, and each shared mailbox below as
its own collapsible tree, read-only ones marked with a lock. Click any folder —
yours or a shared mailbox's — to open it; everything (open, reply, move, flag)
then acts on whichever mailbox that folder belongs to, and folder management
(create/rename/delete/colour) applies to the mailbox you're currently in.

When you compose or reply from within a shared mailbox, the message is sent
**as** (or **on behalf of**) that address, and the sent copy is filed in the
**shared mailbox's** Sent folder — so the whole team can see what went out.

---

## Isolation & safety

Delegation never weakens account isolation (the product's first rule):

- A grant is **scoped to one tenant** — a delegate can never reach a mailbox in
  another organisation.
- Access you weren't granted is indistinguishable from a mailbox that doesn't
  exist (no "it exists but you can't see it" hint).
- A delegated session **never** gains admin rights.
- Revoking access takes effect immediately.

## Current limits

- Per-folder grants apply to whole folders (and their subfolders); there are no
  finer per-message or per-sender rules.
- Live cross-member updates land within a moment on any connected client; a
  client that was fully offline catches up when it reconnects.
