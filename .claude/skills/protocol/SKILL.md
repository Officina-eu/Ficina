---
name: protocol
description: Protocol-correctness reference for Ficina. Consult for ANY work touching SMTP, JMAP, IMAP, POP3, Sieve, CalDAV/CardDAV, iCalendar, DKIM/SPF/DMARC/ARC, or wire formats — implementing commands, parsing, reply codes, or debugging client behavior. Triggers on RFC numbers, protocol names, "reply code", "header", "parsing", or interop bugs.
---

# Protocol work

Mail protocols are 40 years of law plus 40 years of case law (what
clients actually do). We honor both, in that order.

## Doctrine

1. **Strict in what we send.** Our output is RFC-exact: correct reply
   codes, correct folding, correct timeouts. We are never the peer
   others need a quirk for.
2. **Tolerant in what we accept** — within safety. Real clients send
   garbage; parse defensively, never crash on malformed input, and
   *reject* (do not guess) when ambiguity has security consequences
   (smuggling, header injection, path traversal).
3. **Every deviation is recorded.** When a mainstream client forces
   behavior beyond the RFC, implement the accommodation and log it in
   `docs/interop.md`: client, version, quirk, our response, date.
4. **Cite before you code.** Every protocol behavior in a PR names
   its RFC section. "I think SMTP does X" is not a citation.

## The law library

| Area | Primary RFCs |
|---|---|
| SMTP | 5321 (protocol), 5322 (message format), 6409 (submission), 3463 (status codes) |
| SMTP extensions | 1870 (SIZE), 2920 (PIPELINING), 3207 (STARTTLS), 4954 (AUTH), 6152 (8BITMIME), 6531 (SMTPUTF8) |
| Auth stack | 6376 (DKIM), 7208 (SPF), 7489 (DMARC), 8617 (ARC), 8461 (MTA-STS), 7672 (DANE) |
| JMAP | 8620 (core), 8621 (mail), 8887 (websocket) |
| IMAP | 9051 (IMAP4rev2), 3501 (rev1 — what most clients still speak), 2177 (IDLE) |
| POP3 | 1939 |
| Sieve | 5228 (base), 5229/5230/5233 (variables, vacation, subaddress) |
| MIME | 2045–2049, 2231 (encoded params) |
| Calendar/contacts | 4791 (CalDAV), 6352 (CardDAV), 5545 (iCalendar), 5546 (iTIP), 6047 (iMIP), 6638 (scheduling) |
| Lists/unsubscribe | 2369 (List-* headers), 8058 (one-click) |

## Non-negotiables

- CRLF is the line ending. Always. Both directions.
- Reply codes are semantic: 4xx means retry-later, 5xx means never —
  choosing wrong causes silent mail loss or infinite retries.
- Size limits enforced *during* read, not after buffering.
- Header injection: any user-influenced value entering a header is
  validated against CR/LF before writing.
- Timeouts per RFC 5321 §4.5.3.2 — a stuck peer must not hold a
  worker forever.
- 8-bit/UTF-8 correctness end to end (6152/6531) — we serve Europe;
  "Müller" and "Liège" are test cases, not edge cases.

When the RFC is genuinely ambiguous: match Postfix's observable
behavior (the de-facto reference) and record the choice in
`docs/interop.md`.
