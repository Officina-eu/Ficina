# Spam filtering + MTA-STS (design note)

Finishes the two unapproved M4 deferrals before M5: (1) Rspamd spam
scoring consulted at DATA, and (2) MTA-STS policy serving. ARC,
TLS-RPT *reporting*, and DMARC report *delivery* stay deferred (approved
cut seams, tracked in ROADMAP).

## 1. Rspamd at DATA

### Surface

- **Input:** the fully-received message plus envelope context (client
  IP, HELO, MAIL FROM, RCPT TOs). Consulted only on the MX role, after
  SPF/DKIM/DMARC, before the `250`.
- **Transport:** an HTTP `POST /checkv2` to Rspamd's controller
  (`ALO_SMTP_RSPAMD_URL`, e.g. `http://127.0.0.1:11333`). The raw
  message is the body; envelope fields ride as `IP`/`Helo`/`From`/
  `Rcpt`/`MTA-Name` request headers (Rspamd's documented metadata
  protocol). All attacker-controlled header values are CR/LF-stripped
  before they enter the request — no HTTP header injection into the
  scanner call.
- **Output → reply codes** (RFC 5321 codes; Rspamd action → verdict):
  - `reject` → **550** 5.7.1 (message refused as spam).
  - `soft reject` / `greylist` → **451** 4.7.1 (temporary defer, sender
    retries).
  - `add header` / `rewrite subject` / `no action` → **accept**; the
    result and score are recorded as an `x-spam` method in
    `Authentication-Results` (the "why was this flagged" data,
    features.md [L]).
- **Errors:** Rspamd unreachable / HTTP error / unparseable JSON /
  timeout → **451 fail-closed** (see rejected alternative). A malformed
  response never panics — it maps to the tempfail verdict. The consult
  runs **independent of the DNS resolver**: if SPF/DKIM/DMARC are
  disabled because the system resolver failed to initialize, a
  configured scanner is still consulted (and still fails closed), so a
  transient resolver outage cannot silently drop spam filtering.

### Tenancy

No tenant store is touched. Rspamd scans the single in-flight message;
the verdict affects only that transaction. (Per-tenant spam policy is a
store/admin concern for a later phase.)

### Rollout

Off by default: with `ALO_SMTP_RSPAMD_URL` unset there is no scanner
and mail flows unscanned (dev/receive-only). Fail-closed applies only
when a scanner *is* configured — configuring one and having it down must
not silently disable filtering. Watch-metric: rate of 451/550 spam
verdicts and Rspamd call errors (logged at `error`). Off-switch: unset
the URL.

### Out of scope

Per-tenant fail-open, greylist state (we defer to Rspamd's own greylist
module verdict), Rspamd's own DKIM/ARC signing, and bayes training UI.

## 2. MTA-STS policy serving

### Surface

- **Render** (in `alo-auth-mail::mta_sts`): a validated
  `MtaStsPolicy { mode, mx[], max_age, id }` produces the RFC 8461 §3.2
  policy text and the `_mta-sts` DNS TXT value (`v=STSv1; id=…`). The
  `id` is derived from the policy content by default, so it changes iff
  the policy changes (RFC 8461 §3.1 requires a new id on change).
- **Serve** (in `alo-smtp::mta_sts`): a minimal HTTP responder bound
  to `ALO_SMTP_MTA_STS_ADDR` answering `GET
  /.well-known/mta-sts.txt` with the rendered policy (200, `text/plain`),
  404 for any other path, 405 for non-GET. Plaintext behind the deploy
  TLS-terminating proxy — RFC 8461 requires HTTPS with a WebPKI-valid
  cert on `mta-sts.<domain>`, which is the proxy's job, documented in
  `docs/interop.md` and `deploy/.env.example`.

### Tenancy

Per-deployment, not per-tenant: the policy names this server's hostname
/ MX patterns. No store touched.

### Rollout

Off by default (no `ALO_SMTP_MTA_STS_ADDR` → not served). Enabling is
a config change plus publishing two DNS records (documented); disabling
is unsetting the addr and dropping the `_mta-sts` TXT to `id` change.

### Out of scope

TLS-RPT report *generation/collection* (`_smtp._tls` reporting) — a
sanctioned cut seam; only the MTA-STS side ships here. ~~DANE/TLSA~~
(since built: outbound DANE lives in `alo-smtp-client::dane` +
`alo-smtp::resolver`, see the trust-stack design note).

## Rejected alternatives

- **Rspamd fail-open on unreachable.** Rejected as default: a scanner
  outage is the worst moment to accept everything. Default fail-closed
  (451, sender retries); per-tenant fail-open is a later operator
  opt-in. (Restates the M4 note.)
- **A full HTTP client crate (reqwest/hyper) for the Rspamd call.**
  Rejected: the call is one `POST` to a localhost/container endpoint
  over plaintext. A ~40-line purpose-built client (`Connection: close`,
  read-to-EOF, `serde_json` for the body) avoids a large dependency
  tree and its own advisory surface, and keeps the fail-closed/timeout
  policy ours. Same reasoning for the MTA-STS responder (one static
  route).
- **Serve the MTA-STS policy as a static file from the reverse proxy
  only.** Rejected: rendering-and-serving from our config keeps the
  policy `id` and `mx` list in one authoritative place (the SMTP
  config), so operators can't publish a policy that disagrees with the
  running server; the proxy still terminates TLS.
