# alo Chat — design (ADR 0038)

Channels, DMs, threads and reactions on alo's own store. This note is the
contract the build follows, phase by phase. Domain references (UX law 2):
**Slack and WhatsApp** for reflexes, **Sila (silahq.com)** for the visual bar
and the agent-native feel.

## Surface

All routes authenticated, tenant-scoped through the account door, under
`/chat/*` (a new top-level prefix — the production Caddyfile needs it added
at the next deploy; noted for the human, never edited by a builder).

| Route | Does |
|---|---|
| `GET /chat/channels` | The caller's channels and DMs, each with unread count and last message preview |
| `POST /chat/channels` | Create a channel `{name, topic?, visibility}` — or open a DM `{kind:"dm", with:[user_id]}` (idempotent: the same pair returns the existing DM) |
| `GET /chat/channels/{id}` | One channel with its members |
| `PATCH /chat/channels/{id}` | Rename / retopic / archive |
| `POST /chat/channels/{id}/members` · `DELETE …/members/{user}` | Join, invite, leave, remove |
| `GET /chat/channels/{id}/messages?before={seq}&limit=` | A page of history, newest-first, `seq`-paginated |
| `POST /chat/channels/{id}/messages` | Post `{body, thread_root?, attachments?:[drive_node_id]}` → the stored message with its `seq` |
| `PATCH /chat/messages/{id}` · `DELETE /chat/messages/{id}` | Edit (keeps `edited_at`) · soft-delete (tombstone, never a hole in the sequence) |
| `POST /chat/messages/{id}/reactions` | Toggle `{emoji}` for the caller |
| `POST /chat/channels/{id}/read` | Advance read state `{seq}` |

Live delivery: writes publish to the **existing RFC 8620 EventSource push
hub** (`state.push.publish`) with new types `ChatMessage` and `ChatChannel`,
addressed to each member's stream; clients refetch the affected channel. No
WebSocket layer, no second pipe.

## Data model

- `chat_channels` — id, tenant_id, kind (`channel`|`dm`), name (null for DM),
  topic, visibility (`public`|`private`), created_by, created_at, archived_at.
- `chat_members` — tenant_id, channel_id, user_id, role (`owner`|`member`),
  joined_at, **last_read_seq**, muted. Membership is the permission.
- `chat_messages` — id, tenant_id, channel_id, **seq** (per-channel monotonic,
  allocated in the write transaction), author_id, body, kind (`text`|`system`),
  thread_root_seq (null = top level), edited_at, deleted_at, created_at.
- `chat_reactions` — tenant_id, message_id, user_id, emoji (one row per
  person per emoji; toggling deletes it).
- `chat_attachments` — message_id → **Drive node id**: a pointer, never a
  copy (the Spaces precedent; one storage, per ADR 0038).
- `chat_mentions` — message_id, user_id: written at post time so unread and
  "mentions me" badges are a cheap query, not a text scan.

Sequence allocation reuses the pattern already proven twice in this codebase
(mailbox UIDs, gapless invoice numbers): a per-channel counter row locked in
the same transaction as the insert.

## Errors

| Situation | Answer |
|---|---|
| No token | 401 |
| A channel the caller cannot see (foreign tenant, or private and not a member) | **404 — never 403**: existence is not disclosed, the same language sites and insights use |
| Empty body, body over the limit, unknown emoji, bad attachment id | 422 naming the field, verbatim to the UI (UX law 8) |
| Editing/deleting someone else's message | 403 (the message is visibly not yours — no secret to keep) |
| Posting to an archived channel | 422 with the reason |

## Tenancy

Every table carries `tenant_id`; every query runs through `for_account`, so a
foreign id is a clean not-found rather than a denial. **The wrong-tenant test
is mandatory on each phase that touches storage**: an outsider tenant sees
nothing on every path, a co-member sees the channel, a non-member of a
private channel gets 404. DM channels are additionally invisible to everyone
but their two members.

## Out of scope (v1 — recorded, not forgotten)

E2EE and Matrix federation (ADR 0038 non-goals) · voice/video huddles (that is
alo Meet) · guest access and cross-org channels (a later wave) · presence and
typing indicators (a later phase; they need a cheap ephemeral channel, not the
message store) · message workflows and bots · retention policies · full-text
chat search *within* this design — history search rides the existing workspace
index when the chat corpus is registered with it.

## Rejected alternatives

1. **Synapse/Matrix** (ADR 0003's original choice) — rejected in ADR 0038:
   its federation and E2EE are unusable beside agents and server-side search,
   it needs a parallel media store, and it costs a server per tenant.
2. **WebSockets for live delivery** — rejected: the RFC 8620 EventSource
   stream already exists, already carries mail, is already proxied, and the
   web client already subscribes to it. Sending is an ordinary POST; adding a
   second transport would double the failure modes for no capability.
3. **Timestamps as the ordering key** — rejected: clock skew and equal
   timestamps make pagination and read state ambiguous. A per-channel
   sequence is exact, and the codebase already knows the pattern.
