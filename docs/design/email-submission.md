# Design note — sending mail (JMAP EmailSubmission → SMTP)

Status: building · 2026-07 · ROADMAP Phase 2 "Mail: … compose/reply" and the
deferred `EmailSubmission/set` seam.

The web app can read mail; it cannot send. This wires the JMAP layer
(`alo-jmap`, what the web app talks to) to the SMTP outbound path
(`alo-smtp`, what delivers) so a composed/replied message actually leaves
the server — signed, queued, and delivered like all other outbound mail.

## The seam (and the alternative rejected)

DKIM signing happens **inside the SMTP submission session** (at DATA, before
spooling — `server.rs` `handle_data_phase`), not at delivery. The outbound
queue runner does **not** sign. So a message must travel **through a submission
pipeline** to be signed; writing it straight to the spool would deliver it
**unsigned → DMARC fail → spam/reject**.

**Decision:** `alo-smtp` gains a **trusted internal submission listener** —
a submission-role runtime with the full pipeline (RFC 6409 fixups + DKIM +
`Received:` stamp + spool) but with **AUTH disabled**, bound inside the
container and **never published to the host/internet** (docker-network only).
`alo-jmap` speaks SMTP to it to send. This reuses the entire proven outbound
path (signing, queue, MX delivery, DSN) and keeps the DKIM key in one service.
It is API-based (SMTP is a defined protocol), honoring ARCHITECTURE's "cross-
service communication through defined APIs … never shared tables/storage".

**Rejected — jmap writes directly to the shared spool directory.** It would
force jmap to hold the DKIM key and re-implement signing/fixups, and couples
two services through shared filesystem storage (the concern the "no shared
tables" rule exists to prevent). The submission API is the clean seam.

## Security (this is the open-relay surface — reviewed by security-auditor)

Two independent controls, either sufficient, both required:

1. **Network isolation.** The internal listener's port is **not** in any
   compose `ports:` mapping, so it is reachable only from other containers on
   the private `alo` network — never from the internet. (Verified after
   deploy: the port does not listen on the host.)
2. **Send-as binding.** `alo-jmap` sets the envelope `MAIL FROM` to the
   **authenticated user's own** canonical address or a registered alias
   (`TenantStore::email_of` / `aliases_of`), rejecting any other From
   (`forbiddenFrom`). A bearer token cannot send as another identity.

The listener trusts its (in-network) caller to have authenticated the user —
the standard "trusted internal MSA" pattern. It is not an open relay: it is not
internet-reachable, and its only caller enforces the From binding.

## Surface

- **New config:** `ALO_SMTP_INTERNAL_SUBMISSION_ADDR` (e.g. `0.0.0.0:2526`),
  `None` disables. `alo-jmap` gets `ALO_JMAP_SUBMISSION_ADDR`
  (host:port of that listener, e.g. `alo-smtp:2526`).
- **JMAP capability:** advertise `urn:ietf:params:jmap:submission`.
- **`Email/set` create** (extended, minimally): accept the fields a real
  outgoing/draft message needs — `subject`, `from`, full `to` + `cc`, a text
  body (`bodyValues`/`textBody`), and `header:In-Reply-To`/`header:References`
  for replies — building a proper RFC 5322 message (all header values
  CR/LF-stripped: no header injection). Stored as a `$draft` in Drafts.
- **`EmailSubmission/set` create** (RFC 8621 §7): `{emailId, identityId?,
  envelope?}`. Resolves the draft's bytes, validates `mailFrom` = the user's
  address/alias, derives `rcptTo` from the envelope or the message's To/Cc,
  submits over the internal listener, and on success removes `$draft`, files
  the message to **Sent**, and marks it `$seen`. `onSuccessUpdateEmail` /
  `onSuccessDestroyEmail` supported minimally.

## Errors

- Unknown/again-foreign `emailId` → `notFound` set-error.
- `mailFrom` not the user's address/alias → `forbiddenFrom`.
- Empty recipient set → `noRecipients`.
- Internal listener unreachable / 4xx-5xx at submit → `forbiddenToSend` with a
  server-side `tracing` error (never leaking recipient/body into the wire
  error). No message is half-sent: submission is one SMTP transaction.

## Tenancy

Every step is tenant+user-scoped by construction: the draft is read through the
token's `AccountStore`; the From address comes from *that* user's
`email_of`/`aliases_of`; the wrong-tenant suite is extended to
`EmailSubmission/set` (tenant A cannot submit tenant B's `emailId` — it is
`notFound`, not data).

## Out of scope (recorded)

- Attachments and HTML-body composition (text/plain first; additive later).
- `Identity/get` with multiple identities — the first slice validates From
  server-side against the account's own addresses; a real `Identity/get` is a
  follow-up.
- Bcc handling nuance, `EmailSubmission/query|changes`, delivery-status
  (`dsn`/`mdn`) requests, send-later/undo (queued-but-held) — later items.
- App-specific passwords / XOAUTH2 for legacy clients (separate seam).

## Security review (security-auditor) — outcomes

Fixed in this change:

- **From-header spoofing (HIGH).** Send-as validation now covers the *visible*
  `From:` header, not only the SMTP envelope: `create_one` parses the draft's
  `From` addr-spec and requires it in the account's own address set
  (`forbiddenFrom` otherwise), so a bearer token cannot send a DKIM-signed
  message as a forged author.
- **Only drafts are sendable (LOW).** Submission requires the `$draft` keyword,
  so a received/sent message cannot be re-sent.
- **Partial-acceptance double-send (LOW).** A submission is success once the
  relay accepts *any* recipient (the message is spooled at that point), so a
  client retry cannot double-send.
- **Recipient cap (MEDIUM, partial).** At most `MAX_RECIPIENTS` (100) per
  submission.

Verified sound by the audit: tenant isolation (draft read + From derive are
account-scoped), SMTP-command and MIME-header injection (both neutralized),
log hygiene (no addresses/bodies/credentials logged), and that the no-auth
relay behaviour is scoped to the internal listener only (587/465/25 keep
AUTH+TLS; 2526 is unpublished).

Tracked residual risks (see `docs/design/security-audit-followups.md`): a
per-user/per-tenant **send-rate quota** (the recipient cap is not a rate
limit), and the internal listener binding `0.0.0.0` on the shared docker
network (defence-in-depth against a *co-container* compromise — the internet-
facing controls are unaffected; the recommended fix is a **Unix-domain-socket**
submission channel, eliminating the TCP surface entirely).
