# Sieve filtering (design note)

Sieve (RFC 5228) scripts are **user-supplied programs executed on the
server at delivery time**. That single fact sets the tone of this
milestone: it is a sandboxing exercise wearing a protocol's clothes. Every
limit below is a security control — a script author is an adversary who may
also be a victim (a runaway or hostile script must never lose the user's
mail, amplify into a redirect storm, or reach another account).

## Shape — three parts, one direction

- **`ficina-sieve` — the engine, pure and protocol-agnostic.** It compiles
  script text into an AST (enforcing `require` and hard parse limits) and
  evaluates a compiled script against a message, returning an **outcome**:
  an ordered list of [`Action`]s (keep / fileinto / discard / redirect /
  vacation, plus flag sets) and any warnings. It performs **no I/O**: it
  reads a message and a small evaluation context and returns data. It knows
  nothing about the store, the queue, JMAP, or ManageSieve.
- **The store owns state and filing.** `AccountStore` stores each account's
  scripts (compile-checked on write) and the vacation-suppression table,
  and its `deliver_sieve(raw)` runs the engine and performs the **store-side**
  actions itself — keep/fileinto/discard and flag application — inside the
  same isolation door as the rest of a user's mail. Actions that need the
  outbound path (redirect, vacation reply) it does **not** perform; it
  returns them to the caller as `OutboundAction`s (after taking the
  store-side safety decisions: vacation suppression, redirect budget).
- **The delivery bridge performs outbound actions.** Local delivery
  (SMTP → mailbox) resolves a recipient to an account, calls `deliver_sieve`,
  and hands the returned `OutboundAction`s to the SMTP outbound queue with
  the rule owner's submission identity. The store never depends on the
  queue; the bridge is the only place both meet. **This bridge is M5 (local
  delivery) and is deferred** — SMTP currently spools inbound mail rather
  than delivering it into the store (see ROADMAP "Receiving & sending").
  The seam is ready: `Store::account_by_email` resolves a recipient and
  `AccountStore::deliver_sieve` is the exact entry M5 will call. Until then,
  the Sieve delivery path is exercised through that entry directly (the
  migration tool and tests deliver through it), not through a live SMTP
  session.

This split is deliberate: the engine is unit-testable without a database,
the store keeps isolation and durability, and the one component that can
send mail on a user's behalf (the bridge) is small and auditable.

## Where in delivery it runs (§2.10)

`deliver_sieve` runs **after** the trust stack and spam scoring have
stamped their headers (so a script may test `Authentication-Results` /
`X-Spam-*`) and **before** any mailbox filing. Implicit keep (RFC 5228
§2.10.2) is the default: if no `fileinto`/`keep`/`discard` fired, the
message goes to the Inbox. **No script error ever loses mail** (§2.10.6):
a compile error on the *active* script, a runtime failure, an execution-
budget overrun, or an action that cannot be performed all fall back to
implicit keep into the Inbox, with a logged warning — never a bounce, never
a drop.

## Limits are security controls (enforced at parse, not after)

- **Script size** ≤ 64 KiB, **nesting depth** ≤ 15, **test-list length**
  ≤ 64, **string literal** ≤ 16 KiB — all checked *during* parse so a
  hostile script is rejected before an AST is built, not after.
- **Execution budget**: an instruction counter bounds evaluation; exceeding
  it aborts to implicit keep (a `while`-free language still needs this
  because `:matches` glob and long test lists cost real time).
- **`require` is enforced**: using a command/test/comparator from an
  undeclared extension is a compile error (RFC 5228 §3.2). We support a
  fixed capability set (`fileinto`, `envelope`, `vacation`, `subaddress`,
  `imap4flags`, `comparator-i;ascii-numeric`); anything else `require`d is
  an `unsupportedExtension` compile error.
- **`:matches` glob** uses a single-star-anchor two-pointer match — worst
  case O(pattern×value), **no exponential backtracking** — bounded by the
  16 KiB string-literal and message-size limits.

## Redirect safety — a storm must be impossible by construction

`redirect` is the one action that turns one inbound message into outbound
mail, so it is bounded on every axis:

1. **Per-script redirect count**: at most **N (=3)** `redirect` actions are
   emitted per evaluation; beyond that the redirect is dropped with a
   warning. A script cannot fan one message out to a crowd.
2. **Per-account redirect rate**: the store counts redirects per account per
   rolling window (`redirect_budget`, a store table); over the ceiling, the
   redirect is suppressed (message still delivered) and logged. One
   account's compromised script cannot become a relay.
3. **Loop prevention**: a message is **not** redirected if it already
   carries `Auto-Submitted:` (anything but `no`), an empty return-path
   (`MAIL FROM:<>`), or a `Received:` count over a ceiling (a message that
   has already been forwarded many times is dropped from further
   redirection). The redirected copy adds a `Received:` so the ceiling bites
   on the next hop.
4. **No self-redirect trivial loop**: redirect to the rule owner's own
   address is refused (it would re-enter delivery and re-run the script).

**Deferred hardening (recorded, not yet built):** a per-account *vacation*
send budget mirroring the redirect budget — today vacation is bounded only
per-correspondent (`:days`), which stops a flood at one victim but not one
reply each to many spoofed-but-owner-addressed correspondents (1:1, no
amplification, and the outbound path is M5-deferred, so this is bounded).
And the self-redirect / vacation-self checks match only the account's
primary address; they widen to cover aliases when ficina-identity lands
aliases. **M5 must-do:** the outbound bridge that turns
`OutboundAction`'s `subject`/`from`/`address` into headers/envelope MUST
CR/LF-validate them first (a script string may carry raw CR/LF) — no
current code writes them into a header.

**Rejected redirect-safety alternative — trust the script and rely on the
outbound queue's own rate limits.** Rejected: the queue limits *destinations*,
not *amplification at the source*; a script that redirects every inbound
message to one external address is a perfect reflector the queue would
happily drain. Amplification must be capped where it originates — in the
engine's per-script count and the store's per-account budget — before a
single message reaches the queue.

## Vacation (RFC 5230) and its RFC 3834 guard rails

`vacation` replies are auto-responses, the classic backscatter source, so:

- **Never auto-reply** to a message with `Auto-Submitted:` (≠ `no`), a
  `List-Id`/`List-Unsubscribe`/`List-*` header, a null return-path, a
  `Precedence: bulk/list/junk`, or one whose `From`/envelope-from is the
  responder's **own** address (RFC 3834 §2). The engine makes this decision
  from the message alone.
- **Per-correspondent suppression** (`:days`, default 7): the store keeps a
  `(account, correspondent, handle)` → last-sent table; a second message
  from the same correspondent within `:days` sends no reply. `:handle`
  scopes the suppression; `:subject`/`:from`/`:addresses` shape the reply.
- The reply carries **`Auto-Submitted: auto-replied`** (RFC 3834 §5),
  `In-Reply-To`/`References` to the triggering message, an empty envelope
  return-path (so it can never itself trigger a bounce loop), and is sent
  through the same bounded outbound path as redirect.

## fileinto — auto-create is OFF

`fileinto "Folder"` files into an existing mailbox resolved by IMAP-style
name. A **non-existent** target is not created (auto-create off, per the
milestone constraint): the action degrades to implicit **keep into Inbox**
and records a script warning. This avoids a typo silently spawning folders
and a hostile script inflating a mailbox tree.

## subaddress (RFC 5233) & imap4flags (RFC 5232)

`subaddress` reads `:user`/`:detail` off the `user+detail@domain` form the
store already accepts on delivery, so `:detail "tag"` tests the plus-tag.
`imap4flags` (`setflag`/`addflag`/`removeflag`) maps IMAP flags to the
store's JMAP keywords and applies them to the filed message.

## Isolation

Scripts, suppression rows, and redirect budgets are per-account rows on
`AccountStore`; every read and write carries `(tenant, user)` by
construction. A script can `fileinto` only the **owner's** mailboxes
(name resolution runs against the owner's `AccountStore`), and one
account's script can never run on, file into, or read another account's
mail. The isolation suite extends to script CRUD **and** to execution.

## Out of scope (recorded)

ManageSieve (ADR 0007 — additive later); the visual rule builder (Phase 2);
`extlists`, `mailbox`/`mboxmetadata`, `spamtest`/`virustest`, `include`,
`editheader`, `regex`, `variables`, `relational`, `date`/`index`,
`notify`, `ihave`, `body` (deep MIME body search) — additive extensions
named so their absence is a decision. `body` in particular is deferred:
matching the raw decoded body needs the bounded MIME walk and is a
follow-up; scripts test headers/envelope/size now.
