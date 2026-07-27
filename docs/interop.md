# Interop log

Client quirks and RFC deviations. Format per entry: date · client+version · quirk observed · our response · RFC section affected.

(no client-forced entries yet — and every entry added here is a debugging session nobody repeats)

## Standing policies (deliberate strictness/tolerance choices, not client-forced)

- 2026-07-25 · **Bare LF / stray CR rejected everywhere** · RFC 5321 §2.3.8 requires
  CRLF; bare line endings are the SMTP-smuggling vector. Commands: 500, session
  continues. Inside DATA: 500 and the connection **closes** (the stream cannot be
  trusted to re-sync). Matches Postfix `smtpd_forbid_bare_newline=yes` posture.
- 2026-07-25 · **One space tolerated after `MAIL FROM:`/`RCPT TO:`** · RFC 5321
  §4.1.1.2 admits no space; many clients send one; no security ambiguity → accepted.
- 2026-07-25 · **DATA line length** · §4.5.3.1.6 sets 1000 octets as the sender
  limit; long HTML lines are routine in real mail. We accept content lines up to
  8192 octets and reject the message (500, drained, session survives) beyond —
  tolerance with a defensive ceiling.
- 2026-07-25 · **8-bit bytes accepted in DATA without 8BITMIME advertised** ·
  Strictly, unadvertised 8BITMIME means 7-bit only (RFC 6152); rejecting 8-bit
  bodies would bounce half of real-world mail. Accepted, like every mainstream MTA.
  8BITMIME advertisement lands with the capability milestone (M3).
- 2026-07-25 · **General address literals rejected 501** · §4.1.3 tagged literals
  (`[tag:content]`) are syntactically legal; nothing routable can be done with
  them; IPv4 and `IPv6:` literals are accepted.
- 2026-07-25 · **Source routes accepted and ignored** · §4.1.2 / Appendix C:
  `<@relay:user@dom>` parses, the route is validated then discarded.
- 2026-07-26 · **DATA has a 10-minute total budget** · §4.5.3.2 specifies
  per-wait timeouts; we additionally bound the whole message receive at 600 s as
  anti-flood policy. A legitimate sender slower than ~350 kbps on a 25 MiB
  message would be cut off (421); accepted trade-off, revisit with real traffic.
- 2026-07-26 · **EHLO/HELO argument restricted to printable ASCII** · §4.1.1.1
  expects a Domain/address-literal; we reject control octets with 501 so the
  attacker-controlled greeting can never inject binary into the Received: stamp
  or the spool sidecar. SMTPUTF8 (M3) will widen this to U-labels.
- 2026-07-27 · **Outbound delivery off by default** · M1 accepts any recipient;
  turning on relaying before the AUTH gate (M3) would make an exposed instance
  an open relay. Delivery requires `FICINA_SMTP_OUTBOUND_ENABLED=true`, and a
  smarthost route is the supported self-hosted mode until MX+AUTH are complete.
- 2026-07-27 · **Empty MAIL FROM on outbound = null path** · we send
  `MAIL FROM:<>` for DSNs and never generate a DSN for a message that itself
  arrived with a null reverse-path (RFC 5321 §4.5.5 loop prevention).
- 2026-07-27 · **Domainless recipients parked, not delivered** · a bare
  `<postmaster>` (§4.1.1.3) has no domain to route to; M2 holds such messages
  in the spool (logged) pending local delivery (M5) rather than dropping or
  bouncing them.
- 2026-07-27 · **STARTTLS discards all pre-TLS state** · RFC 3207 §4.2: after a
  successful STARTTLS the HELO/EHLO identity, any transaction, and any prior
  auth are cleared; the client must EHLO again. Buffered plaintext arriving
  after our 220 and before the handshake is treated as a command-injection
  attempt (CVE-2011-0411 class) and the connection is dropped, nothing executed.
- 2026-07-27 · **AUTH offered only on submission over TLS** · `AUTH PLAIN`/`LOGIN`
  are advertised and accepted only on a submission listener with TLS active.
  On the MX (port 25) role AUTH is refused 503; before TLS it is refused 538.
  Wrong password and unknown user return the same 535 (anti-enumeration, §7.3).
- 2026-07-27 · **EHLO capabilities are state-exact** · we advertise STARTTLS only
  while TLS is inactive, AUTH only on submission-over-TLS, and always SIZE and
  8BITMIME. Advertising a capability implies accepting its MAIL parameters, so
  `SIZE=`, `BODY=7BIT|8BITMIME`, and `AUTH=` (RFC 4954 §5, accepted and ignored)
  are honored; every other MAIL parameter is still 555.
- 2026-07-27 · **Submission adds Date/Message-ID only** · RFC 6409 §8 permits the
  MSA to rewrite more, but we make the minimal non-destructive fix (add `Date:`
  and `Message-ID:` when absent) and never touch `From`/`Sender` or the body.
- 2026-07-27 · **Submission requires STARTTLS then AUTH** · on submission
  ports (587/465) MAIL before TLS gets 530 (must STARTTLS) and MAIL before a
  successful AUTH gets 530 (auth required) — the open-relay gate. MX (25)
  authenticates no one and never advertises AUTH.
