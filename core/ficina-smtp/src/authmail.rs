//! Bridge from the SMTP transaction to `ficina-auth-mail`: at DATA on
//! the MX role it runs SPF + DKIM + DMARC and stamps `Received-SPF` and
//! `Authentication-Results` (the RFC 8601 contract) onto the message,
//! applying the DMARC disposition; at submission it DKIM-signs.
//!
//! Kept separate from the transport (`server`) and the pure auth logic
//! (`ficina-auth-mail`) so each has one reason to change.

use std::net::IpAddr;
use std::sync::Arc;

use ficina_auth_mail::authres::AuthenticationResults;
use ficina_auth_mail::dkim::keystore::KeyStore;
use ficina_auth_mail::dkim::{self, Message, SignParams};
use ficina_auth_mail::dmarc::{self, Disposition, DmarcResult};
use ficina_auth_mail::resolver::Resolver;
use ficina_auth_mail::spf::{self, Mailbox, SpfQuery};

/// DKIM signing configuration for the submission path.
pub struct SigningConfig {
    /// The key backend.
    pub keys: Arc<dyn KeyStore>,
    /// Signing domain (`d=`).
    pub domain: String,
    /// Selector (`s=`).
    pub selector: String,
}

/// The trust-stack context attached to a listener. `disabled()` (no
/// resolver, no signer) is the default for tests and for a receive-only
/// dev run; `run` installs a real resolver and, on submission, a signer.
pub struct AuthMail {
    hostname: String,
    resolver: Option<Arc<dyn Resolver>>,
    signing: Option<SigningConfig>,
}

/// The result of the inbound gauntlet: headers to prepend, whether
/// DMARC policy says to reject, and (when a forged header had to be
/// removed) the rewritten message body to spool instead of the original.
pub struct InboundResult {
    /// `Received-SPF` + `Authentication-Results` blocks, each ending in
    /// CRLF, ready to prepend to the message.
    pub headers: String,
    /// True when DMARC disposition is `reject` — the caller sends 550.
    pub reject: bool,
    /// `Some(bytes)` when a pre-existing `Authentication-Results` (with
    /// our authserv-id) or `Received-SPF` header was stripped from the
    /// received message (RFC 8601 §5); the caller must spool these bytes
    /// in place of the original. `None` when nothing was removed.
    pub stripped_body: Option<Vec<u8>>,
}

impl AuthMail {
    /// A context that performs no authentication and no signing.
    pub fn disabled(hostname: impl Into<String>) -> Self {
        Self {
            hostname: hostname.into(),
            resolver: None,
            signing: None,
        }
    }

    /// Installs the DNS resolver used for SPF/DKIM/DMARC lookups.
    #[must_use]
    pub fn with_resolver(mut self, resolver: Arc<dyn Resolver>) -> Self {
        self.resolver = Some(resolver);
        self
    }

    /// Installs the submission DKIM signer.
    #[must_use]
    pub fn with_signing(mut self, signing: SigningConfig) -> Self {
        self.signing = Some(signing);
        self
    }

    /// Whether inbound authentication is active.
    pub fn verifies(&self) -> bool {
        self.resolver.is_some()
    }

    /// Runs SPF + DKIM + DMARC over a received message and returns the
    /// headers to prepend plus the reject decision. When no resolver is
    /// configured, returns empty headers and no rejection.
    pub async fn inbound(
        &self,
        peer_ip: IpAddr,
        helo: &str,
        mail_from: Option<&str>,
        raw_message: &[u8],
    ) -> InboundResult {
        let Some(resolver) = &self.resolver else {
            return InboundResult {
                headers: String::new(),
                reject: false,
                stripped_body: None,
            };
        };
        let resolver = resolver.as_ref();

        // SPF over the connecting IP + HELO + MAIL FROM.
        let mail_from_box = mail_from
            .and_then(split_address)
            .map(|(local, domain)| Mailbox { local, domain });
        let spf_query = SpfQuery {
            ip: peer_ip,
            helo: helo.to_owned(),
            mail_from: mail_from_box.clone(),
        };
        let spf_verdict = spf::check_host(resolver, &spf_query).await;

        // DKIM over the raw message.
        let message = Message::parse(raw_message);
        let dkim_verdicts = dkim::verify(resolver, &message).await;

        // DMARC over the RFC 5322 From domain.
        let from_domain = header_from_domain(&message).unwrap_or_default();
        let dmarc_verdict =
            dmarc::evaluate(resolver, &from_domain, &spf_verdict, &dkim_verdicts).await;

        // Build Received-SPF (RFC 7208 §9.1).
        let identity = mail_from.unwrap_or(helo);
        let identity_key = if mail_from.is_some() {
            "smtp.mailfrom"
        } else {
            "smtp.helo"
        };
        let mut headers = format!(
            "Received-SPF: {} ({}) client-ip={}; envelope-from={}; helo={};\r\n",
            spf_verdict.result.as_str(),
            // The explanation is a parenthesized comment (spaces allowed);
            // envelope-from/helo are `key=value` tokens — strip the
            // structural chars (SP, `;`, `=`) an attacker could use to
            // forge extra Received-SPF key/value pairs.
            sanitize(&spf_verdict.explanation),
            peer_ip,
            sanitize_token(identity),
            sanitize_token(helo),
        );

        // Build Authentication-Results (the contract).
        let mut ar =
            AuthenticationResults::new(&self.hostname).spf(&spf_verdict, identity_key, identity);
        for verdict in &dkim_verdicts {
            ar = ar.dkim(verdict);
        }
        if dmarc_verdict.result != DmarcResult::None {
            ar = ar.dmarc(&dmarc_verdict);
        }
        headers.push_str(&format!("Authentication-Results: {}\r\n", ar.render()));

        // DMARC disposition: reject only on an explicit reject policy,
        // after applying the published `pct` sampling (§6.6.4) so a
        // sender mid-rollout (`p=reject; pct<100`) is not enforced at
        // 100%. The random draw is a non-cryptographic sub-nanosecond
        // sample — sufficient for policy sampling.
        let roll = (jiff::Timestamp::now().subsec_nanosecond().unsigned_abs() % 100) as u8;
        let effective =
            dmarc::sample_disposition(dmarc_verdict.disposition, dmarc_verdict.pct, roll);
        let reject = dmarc_verdict.result == DmarcResult::Fail && effective == Disposition::Reject;
        if reject {
            tracing::info!(from = %from_domain, "DMARC reject policy; refusing message");
        }

        // RFC 8601 §5: strip any pre-existing Authentication-Results
        // (bearing our authserv-id) or Received-SPF header a remote
        // sender may have forged, so downstream never trusts a
        // planted verdict.
        let stripped_body = strip_authserv_headers(raw_message, &self.hostname);

        InboundResult {
            headers,
            reject,
            stripped_body,
        }
    }

    /// DKIM-signs an outbound (submission) message, returning the
    /// `DKIM-Signature:` header line (with CRLF) to prepend, or `None`
    /// when signing is not configured or the key is unavailable.
    pub async fn sign_outbound(&self, raw_message: &[u8]) -> Option<String> {
        let signing = self.signing.as_ref()?;
        let message = Message::parse(raw_message);
        let params = SignParams::new(&signing.domain, &signing.selector);
        match dkim::sign(signing.keys.as_ref(), &message, &params).await {
            Ok(value) => Some(format!("DKIM-Signature: {value}\r\n")),
            Err(error) => {
                // Signing failure must not lose the message; log and
                // send it unsigned (deliverability degrades, mail flows).
                tracing::error!(%error, "DKIM signing failed; sending unsigned");
                None
            }
        }
    }
}

/// Splits a `local@domain` address into its parts (lowercasing the
/// domain). Returns `None` when there is no `@`.
fn split_address(addr: &str) -> Option<(String, String)> {
    let addr = addr.trim().trim_start_matches('<').trim_end_matches('>');
    let (local, domain) = addr.rsplit_once('@')?;
    if local.is_empty() || domain.is_empty() {
        return None;
    }
    Some((local.to_owned(), domain.to_ascii_lowercase()))
}

/// Extracts the domain of the RFC 5322 `From` header for DMARC. Returns
/// `None` when there is no `From`, no parseable domain, or — per RFC
/// 7489 §6.6.1 — the header carries multiple addresses in *different*
/// domains (DMARC alignment then has no single domain to judge, so it
/// must not proceed on an attacker-chosen one).
fn header_from_domain(message: &Message<'_>) -> Option<String> {
    let (_, value) = message
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("From"))?;
    // A `From` may list several mailboxes (comma-separated). Collect the
    // distinct domains; DMARC requires exactly one.
    let mut domains: Vec<String> = Vec::new();
    for part in value.split(',') {
        let Some(at) = part.rfind('@') else { continue };
        let domain: String = part[at + 1..]
            .chars()
            .skip_while(|c| c.is_whitespace())
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-')
            .collect();
        let domain = domain.trim_end_matches('.').to_ascii_lowercase();
        if !domain.is_empty() && !domains.contains(&domain) {
            domains.push(domain);
        }
    }
    match domains.as_slice() {
        [only] => Some(only.clone()),
        _ => None,
    }
}

/// Removes any pre-existing `Authentication-Results` header whose
/// authserv-id equals our hostname, and any `Received-SPF` header, from
/// the received message (RFC 8601 §5). Returns `Some(rewritten)` when at
/// least one header was removed, else `None`. Parses over raw bytes and
/// preserves every other header (including legitimate upstream
/// `Authentication-Results` from a *different* authserv-id) byte-exact.
fn strip_authserv_headers(raw: &[u8], authserv_id: &str) -> Option<Vec<u8>> {
    // Split at the header/body separator; only a well-formed message
    // (with a blank line) is rewritten — otherwise leave it untouched.
    let sep = find_double_crlf(raw)?;
    // `hb` covers every header line including its terminating CRLF (the
    // last header's CRLF is the first half of the CRLFCRLF separator).
    let hb = &raw[..sep + 2];
    let tail = &raw[sep + 2..]; // blank line + body, verbatim
    let n = hb.len();
    let mut out: Vec<u8> = Vec::with_capacity(raw.len());
    let mut removed = false;
    let mut i = 0;
    while i < n {
        // A field spans its start line plus any WSP-led continuations.
        let mut end = crlf_after(hb, i);
        while end < n && (hb[end] == b' ' || hb[end] == b'\t') {
            end = crlf_after(hb, end);
        }
        let field = &hb[i..end];
        if field_is_forged(field, authserv_id) {
            removed = true;
        } else {
            out.extend_from_slice(field);
        }
        i = end;
    }
    if !removed {
        return None;
    }
    out.extend_from_slice(tail);
    Some(out)
}

/// Whether a full header field (name through trailing CRLF) is a
/// pre-existing `Received-SPF`, or an `Authentication-Results` whose
/// authserv-id is our own hostname.
fn field_is_forged(field: &[u8], authserv_id: &str) -> bool {
    let Some(colon) = field.iter().position(|&b| b == b':') else {
        return false;
    };
    let name = &field[..colon];
    if name.eq_ignore_ascii_case(b"Received-SPF") {
        return true;
    }
    if name.eq_ignore_ascii_case(b"Authentication-Results") {
        // The authserv-id is the first token of the value.
        let value = field[colon + 1..].trim_ascii_start();
        let id_end = value
            .iter()
            .position(|&b| matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b';'))
            .unwrap_or(value.len());
        return value[..id_end].eq_ignore_ascii_case(authserv_id.as_bytes());
    }
    false
}

/// Index just past the CRLF that ends the line starting at `from`
/// (or `hb.len()` if the line has no CRLF).
fn crlf_after(hb: &[u8], from: usize) -> usize {
    let mut j = from;
    while j + 1 < hb.len() {
        if hb[j] == b'\r' && hb[j + 1] == b'\n' {
            return j + 2;
        }
        j += 1;
    }
    hb.len()
}

fn find_double_crlf(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|w| w == b"\r\n\r\n")
}

/// Removes control characters (CR/LF) from a value placed in a header.
fn sanitize(value: &str) -> String {
    value.chars().filter(|c| !c.is_control()).collect()
}

/// Sanitizes a `key=value` token field (Received-SPF `envelope-from`,
/// `helo`): keeps only dot-atom / addr-spec characters, dropping SP,
/// `;`, `=`, and control chars an attacker could use to forge extra
/// key/value pairs in the header.
fn sanitize_token(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '@' | '+'))
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn split_address_parses_and_lowercases() {
        assert_eq!(
            split_address("Bob@Example.ORG"),
            Some(("Bob".to_owned(), "example.org".to_owned()))
        );
        assert_eq!(
            split_address("<a@b.test>"),
            Some(("a".to_owned(), "b.test".to_owned()))
        );
        assert_eq!(split_address("no-at-sign"), None);
        assert_eq!(split_address("@nolocal"), None);
    }

    #[test]
    fn from_domain_extraction() {
        let raw = b"From: Alice <alice@Example.com>\r\nSubject: x\r\n\r\nbody\r\n";
        let msg = Message::parse(raw);
        assert_eq!(header_from_domain(&msg), Some("example.com".to_owned()));
    }

    #[test]
    fn multi_from_with_differing_domains_is_none() {
        // RFC 7489 §6.6.1: two From domains → no single domain to align.
        let raw = b"From: a@one.example, b@two.example\r\nSubject: x\r\n\r\nbody\r\n";
        let msg = Message::parse(raw);
        assert_eq!(header_from_domain(&msg), None);
        // But two mailboxes in the SAME domain resolve to that domain.
        let raw2 = b"From: a@same.example, b@same.example\r\n\r\nbody\r\n";
        assert_eq!(
            header_from_domain(&Message::parse(raw2)),
            Some("same.example".to_owned())
        );
    }

    #[test]
    fn strips_forged_authserv_headers_only() {
        let raw = concat!(
            "Received-SPF: pass (forged) client-ip=1.2.3.4;\r\n",
            "Authentication-Results: mx.ficina.test; dmarc=pass header.from=bank.com\r\n",
            "Authentication-Results: upstream.example; spf=pass\r\n",
            "From: alice@example.com\r\n",
            "Subject: hi\r\n",
            "\r\n",
            "body\r\n",
        )
        .as_bytes();
        let cleaned = strip_authserv_headers(raw, "mx.ficina.test").expect("headers removed");
        let text = String::from_utf8(cleaned).unwrap();
        // Our own planted verdict and the Received-SPF are gone.
        assert!(!text.contains("dmarc=pass header.from=bank.com"));
        assert!(!text.contains("Received-SPF:"));
        // The legitimate upstream result (different authserv-id) survives.
        assert!(text.contains("Authentication-Results: upstream.example; spf=pass"));
        // The real message is intact.
        assert!(text.contains("From: alice@example.com"));
        assert!(text.contains("Subject: hi"));
        assert!(text.ends_with("\r\n\r\nbody\r\n"));
    }

    #[test]
    fn strip_returns_none_when_nothing_forged() {
        let raw = b"From: alice@example.com\r\nSubject: hi\r\n\r\nbody\r\n";
        assert!(strip_authserv_headers(raw, "mx.ficina.test").is_none());
    }

    #[tokio::test]
    async fn disabled_context_stamps_nothing() {
        let auth = AuthMail::disabled("mx.ficina.test");
        let result = auth
            .inbound(
                "192.0.2.1".parse().unwrap(),
                "helo",
                Some("a@b.test"),
                b"From: a@b.test\r\n\r\nx",
            )
            .await;
        assert!(result.headers.is_empty());
        assert!(!result.reject);
        assert!(auth.sign_outbound(b"msg").await.is_none());
    }
}
