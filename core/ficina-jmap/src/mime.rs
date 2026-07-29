//! Building an RFC 5322 / MIME `text/plain` message for outgoing mail.
//!
//! European-correct by construction: non-ASCII display names and subjects are
//! emitted as RFC 2047 `B` encoded-words (each ≤75 chars), and a non-ASCII
//! body is base64 so transport is 7-bit clean on any path. Every header value
//! is CR/LF-sanitized before use — there is no header-injection path from a
//! composed field. Header lines are folded to ≤78 columns (RFC 5322 §2.1.1);
//! because non-ASCII is always encoded to ASCII first, folding is byte-safe.
//!
//! Scope: `text/plain` bodies with To/Cc and reply threading headers. HTML
//! alternatives and attachments are a later, additive pass (recorded in
//! `docs/design/email-submission.md`).

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;

/// One address: an optional display name plus the addr-spec.
#[derive(Debug, Clone)]
pub struct Addr {
    pub name: Option<String>,
    pub email: String,
}

/// The fields of an outgoing text/plain message.
pub struct Outgoing {
    pub from: Addr,
    pub to: Vec<Addr>,
    pub cc: Vec<Addr>,
    pub subject: String,
    /// Parent message-ids (bare, no angle brackets) for `In-Reply-To`.
    pub in_reply_to: Vec<String>,
    /// The `References` chain (bare message-ids).
    pub references: Vec<String>,
    pub body_text: String,
    /// The submission hostname, for the `Message-ID` domain if the submission
    /// pipeline does not add one. (It does — this is a belt-and-braces seed.)
    pub message_id_domain: String,
    /// A unique token seeding the `Message-ID` local part.
    pub message_id_token: String,
}

/// Builds the full RFC 5322 message bytes (CRLF line endings).
pub fn build(msg: &Outgoing) -> Vec<u8> {
    let mut headers: Vec<String> = Vec::new();

    headers.push(fold(&format!("From: {}", format_addr(&msg.from))));
    if !msg.to.is_empty() {
        headers.push(fold(&format!("To: {}", format_addr_list(&msg.to))));
    }
    if !msg.cc.is_empty() {
        headers.push(fold(&format!("Cc: {}", format_addr_list(&msg.cc))));
    }
    headers.push(fold(&format!(
        "Subject: {}",
        encode_unstructured(&msg.subject)
    )));
    headers.push(format!(
        "Message-ID: <{}@{}>",
        sanitize(&msg.message_id_token),
        sanitize(&msg.message_id_domain)
    ));
    if let Some(first) = msg.in_reply_to.first() {
        headers.push(fold(&format!("In-Reply-To: {}", angle(first))));
    }
    if !msg.references.is_empty() {
        let refs = msg
            .references
            .iter()
            .map(|r| angle(r))
            .collect::<Vec<_>>()
            .join(" ");
        headers.push(fold(&format!("References: {refs}")));
    }
    headers.push("MIME-Version: 1.0".to_owned());

    let (cte, body) = encode_body(&msg.body_text);
    headers.push("Content-Type: text/plain; charset=utf-8".to_owned());
    headers.push(format!("Content-Transfer-Encoding: {cte}"));

    let mut out = Vec::with_capacity(body.len() + 512);
    for h in &headers {
        out.extend_from_slice(h.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(&body);
    if !out.ends_with(b"\r\n") {
        out.extend_from_slice(b"\r\n");
    }
    out
}

/// Strips CR and LF from a header input (header-injection guard).
fn sanitize(s: &str) -> String {
    s.replace(['\r', '\n'], " ")
}

fn is_ascii_clean(s: &str) -> bool {
    s.bytes().all(|b| (0x20..=0x7e).contains(&b))
}

/// Wraps a bare message-id in angle brackets (idempotent).
fn angle(id: &str) -> String {
    let id = sanitize(id);
    let trimmed = id.trim().trim_start_matches('<').trim_end_matches('>');
    format!("<{trimmed}>")
}

/// An address as a header token: `email`, `"Display Name" <email>`, or with an
/// RFC 2047 encoded phrase when the name is non-ASCII.
fn format_addr(a: &Addr) -> String {
    let email = sanitize(&a.email);
    match &a.name {
        Some(name) if !name.trim().is_empty() => {
            let name = sanitize(name);
            if is_ascii_clean(&name) {
                let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
                format!("\"{escaped}\" <{email}>")
            } else {
                format!("{} <{email}>", encoded_words(&name))
            }
        }
        _ => email,
    }
}

fn format_addr_list(list: &[Addr]) -> String {
    list.iter().map(format_addr).collect::<Vec<_>>().join(", ")
}

/// An unstructured header value (Subject): raw when ASCII, else encoded-words.
fn encode_unstructured(s: &str) -> String {
    let s = sanitize(s);
    if is_ascii_clean(&s) {
        s
    } else {
        encoded_words(&s)
    }
}

/// RFC 2047 `B` encoded-words for a UTF-8 string, each ≤75 chars, split on
/// character boundaries and joined by a folding space so the caller can place
/// them in a header.
fn encoded_words(s: &str) -> String {
    // Each encoded-word: "=?UTF-8?B?" + base64 + "?=". Keep base64 ≤ 60 chars
    // → ≤ 45 source bytes per word (well under the 75-char encoded-word limit).
    const MAX_BYTES: usize = 45;
    let mut words: Vec<String> = Vec::new();
    let mut chunk: Vec<u8> = Vec::new();
    for ch in s.chars() {
        let mut buf = [0u8; 4];
        let encoded = ch.encode_utf8(&mut buf).as_bytes();
        if chunk.len() + encoded.len() > MAX_BYTES && !chunk.is_empty() {
            words.push(format!("=?UTF-8?B?{}?=", B64.encode(&chunk)));
            chunk.clear();
        }
        chunk.extend_from_slice(encoded);
    }
    if !chunk.is_empty() {
        words.push(format!("=?UTF-8?B?{}?=", B64.encode(&chunk)));
    }
    // Encoded-words are separated by folding whitespace (a plain space here;
    // fold() will break the line if it grows too long).
    words.join(" ")
}

/// Chooses the body transfer encoding: 7bit for clean ASCII with short lines,
/// otherwise base64 (wrapped at 76). Line endings are normalized to CRLF.
fn encode_body(text: &str) -> (&'static str, Vec<u8>) {
    let normalized = normalize_crlf(text);
    let ascii = normalized.iter().all(|&b| b < 0x80);
    let short_lines = normalized
        .split(|&b| b == b'\n')
        .all(|line| line.len() <= 990);
    if ascii && short_lines {
        ("7bit", normalized)
    } else {
        let b64 = B64.encode(&normalized);
        let mut wrapped = Vec::with_capacity(b64.len() + b64.len() / 76 * 2 + 2);
        for chunk in b64.as_bytes().chunks(76) {
            wrapped.extend_from_slice(chunk);
            wrapped.extend_from_slice(b"\r\n");
        }
        ("base64", wrapped)
    }
}

fn normalize_crlf(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 16);
    let mut prev_cr = false;
    for &b in text.as_bytes() {
        match b {
            b'\n' => {
                if !prev_cr {
                    out.push(b'\r');
                }
                out.push(b'\n');
                prev_cr = false;
            }
            b'\r' => {
                out.push(b'\r');
                out.push(b'\n');
                prev_cr = true;
            }
            other => {
                out.push(other);
                prev_cr = false;
            }
        }
    }
    out
}

/// Folds an ASCII header line to ≤78 columns at spaces (RFC 5322 §2.2.3),
/// continuation lines beginning with a single space. Inputs are ASCII here
/// (non-ASCII was already encoded), so byte indexing is char-safe.
fn fold(header: &str) -> String {
    const LIMIT: usize = 78;
    if header.len() <= LIMIT {
        return header.to_owned();
    }
    let mut out = String::with_capacity(header.len() + 8);
    let mut line_len = 0usize;
    let mut last_space: Option<usize> = None;
    for ch in header.chars() {
        out.push(ch);
        line_len += 1;
        if ch == ' ' {
            last_space = Some(out.len() - 1);
        }
        if line_len > LIMIT
            && let Some(sp) = last_space
        {
            out.replace_range(sp..sp + 1, "\r\n ");
            line_len = out.len() - (sp + 3);
            last_space = None;
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn addr(name: Option<&str>, email: &str) -> Addr {
        Addr {
            name: name.map(str::to_owned),
            email: email.to_owned(),
        }
    }

    fn base(subject: &str, body: &str) -> Outgoing {
        Outgoing {
            from: addr(Some("Disan"), "disan@namel3ss.com"),
            to: vec![addr(Some("Alice Ng"), "alice@example.eu")],
            cc: Vec::new(),
            subject: subject.to_owned(),
            in_reply_to: Vec::new(),
            references: Vec::new(),
            body_text: body.to_owned(),
            message_id_domain: "namel3ss.com".to_owned(),
            message_id_token: "abc123".to_owned(),
        }
    }

    fn text(msg: &Outgoing) -> String {
        String::from_utf8(build(msg)).unwrap()
    }

    #[test]
    fn ascii_message_is_plain_and_7bit() {
        let s = text(&base("Hello", "Hi there\nline two\n"));
        assert!(s.contains("From: \"Disan\" <disan@namel3ss.com>"));
        assert!(s.contains("To: \"Alice Ng\" <alice@example.eu>"));
        assert!(s.contains("Subject: Hello\r\n"));
        assert!(s.contains("Content-Transfer-Encoding: 7bit"));
        assert!(s.contains("\r\n\r\nHi there\r\nline two\r\n"));
    }

    #[test]
    fn non_ascii_subject_is_encoded_word() {
        let s = text(&base("Ründtür — café", "body"));
        assert!(
            s.contains("Subject: =?UTF-8?B?"),
            "subject not encoded: {s}"
        );
        assert!(!s.contains("Ründtür"));
    }

    #[test]
    fn non_ascii_body_is_base64() {
        let s = text(&base("s", "Voilà, c'est déjà prêt — café ☕"));
        assert!(s.contains("Content-Transfer-Encoding: base64"));
        assert!(!s.contains("Voilà"));
    }

    #[test]
    fn non_ascii_display_name_is_encoded() {
        let mut m = base("s", "b");
        m.to = vec![addr(Some("Hélène Fonck"), "helene@proceq.eu")];
        let s = text(&m);
        assert!(s.contains("=?UTF-8?B?") && s.contains("<helene@proceq.eu>"));
        assert!(!s.contains("Hélène"));
    }

    #[test]
    fn header_injection_is_neutralized() {
        let mut m = base("Hi\r\nBcc: evil@x", "b");
        m.to = vec![addr(Some("A\r\nX: y"), "a@x.eu")];
        let s = text(&m);
        // No injected header lines: the only Bcc/X: text is inline, folded away.
        assert!(!s.contains("\r\nBcc: evil@x"));
        assert!(!s.contains("\r\nX: y"));
    }

    #[test]
    fn reply_headers_present_and_bracketed() {
        let mut m = base("Re: hi", "b");
        m.in_reply_to = vec!["orig@a.eu".to_owned()];
        m.references = vec!["<root@a.eu>".to_owned(), "orig@a.eu".to_owned()];
        let s = text(&m);
        assert!(s.contains("In-Reply-To: <orig@a.eu>"));
        assert!(s.contains("References: <root@a.eu> <orig@a.eu>"));
    }

    #[test]
    fn long_recipient_list_folds_under_998() {
        let mut m = base("s", "b");
        m.to = (0..20)
            .map(|i| {
                addr(
                    Some(&format!("Person Number {i}")),
                    &format!("person{i}@example.eu"),
                )
            })
            .collect();
        let s = text(&m);
        for line in s.split("\r\n") {
            assert!(line.len() <= 998, "line exceeds 998: {}", line.len());
        }
    }

    #[test]
    fn plain_address_without_name() {
        let mut m = base("s", "b");
        m.to = vec![addr(None, "bare@example.eu")];
        let s = text(&m);
        assert!(s.contains("To: bare@example.eu\r\n"));
    }
}
