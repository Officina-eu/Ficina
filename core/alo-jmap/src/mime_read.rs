//! Reading inbound MIME. Extracts the display body (plain text + HTML) and
//! the attachment list from a raw RFC 5322 message, decoding transfer
//! encodings (base64/quoted-printable) and charsets along the way. This is
//! delegated to `mail-parser` — reading arbitrary real-world mail correctly
//! (nested multipart, encoded words, charsets) is a parser's job, not a
//! hand-rolled split on the first blank line.

use mail_parser::{MessageParser, MimeHeaders};

/// One attachment surfaced to JMAP: its position among the message's
/// attachments (used to build the composite download blob id), a display
/// name, MIME type, and decoded size in bytes.
pub struct Attachment {
    pub index: usize,
    pub name: String,
    pub content_type: String,
    pub size: usize,
}

/// The reading view of a parsed message.
pub struct Parsed {
    pub text: Option<String>,
    pub html: Option<String>,
    pub attachments: Vec<Attachment>,
}

fn content_type_of(part: &mail_parser::MessagePart) -> String {
    match part.content_type() {
        Some(ct) => match ct.subtype() {
            Some(sub) => format!("{}/{}", ct.ctype(), sub),
            None => ct.ctype().to_owned(),
        },
        None => "application/octet-stream".to_owned(),
    }
}

fn name_of(part: &mail_parser::MessagePart, index: usize) -> String {
    part.attachment_name()
        .map(str::to_owned)
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| format!("attachment-{}", index + 1))
}

/// Parse a raw message into its text/HTML body and attachment list. A message
/// that fails to parse yields an empty view (no body, no attachments) rather
/// than an error — the caller still has headers to show.
pub fn parse(raw: &[u8]) -> Parsed {
    let Some(message) = MessageParser::default().parse(raw) else {
        return Parsed {
            text: None,
            html: None,
            attachments: Vec::new(),
        };
    };
    let text = message.body_text(0).map(|c| c.into_owned());
    let html = message.body_html(0).map(|c| c.into_owned());
    let attachments = message
        .attachments()
        .enumerate()
        .map(|(index, part)| Attachment {
            index,
            name: name_of(part, index),
            content_type: content_type_of(part),
            size: part.contents().len(),
        })
        .collect();
    Parsed {
        text,
        html,
        attachments,
    }
}

/// The decoded bytes of the `index`-th attachment, plus its MIME type and
/// display name — for the download route. `None` if the message doesn't parse
/// or the index is out of range.
pub fn attachment_bytes(raw: &[u8], index: usize) -> Option<(Vec<u8>, String, String)> {
    let message = MessageParser::default().parse(raw)?;
    let part = message.attachments().nth(index)?;
    Some((
        part.contents().to_vec(),
        content_type_of(part),
        name_of(part, index),
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // A multipart/mixed message with a text part and a base64 zip attachment —
    // the shape of a DMARC aggregate report, which previously rendered as raw
    // base64 in the body. "UEsDBAo=" decodes to the bytes PK\x03\x04\n.
    const MSG: &[u8] = concat!(
        "From: a@example.com\r\n",
        "To: b@example.com\r\n",
        "Subject: Report\r\n",
        "MIME-Version: 1.0\r\n",
        "Content-Type: multipart/mixed; boundary=\"b\"\r\n",
        "\r\n",
        "--b\r\n",
        "Content-Type: text/plain; charset=utf-8\r\n",
        "\r\n",
        "Please find the report attached.\r\n",
        "--b\r\n",
        "Content-Type: application/zip; name=\"report.zip\"\r\n",
        "Content-Transfer-Encoding: base64\r\n",
        "Content-Disposition: attachment; filename=\"report.zip\"\r\n",
        "\r\n",
        "UEsDBAo=\r\n",
        "--b--\r\n",
    )
    .as_bytes();

    #[test]
    fn extracts_text_body_not_the_attachment() {
        let parsed = parse(MSG);
        let text = parsed.text.expect("a text body");
        assert!(text.contains("Please find the report attached."));
        assert!(
            !text.contains("UEsDBA"),
            "base64 must not leak into the body"
        );
    }

    #[test]
    fn lists_the_attachment_with_name_and_type() {
        let parsed = parse(MSG);
        assert_eq!(parsed.attachments.len(), 1);
        let a = &parsed.attachments[0];
        assert_eq!(a.name, "report.zip");
        assert_eq!(a.content_type, "application/zip");
        assert_eq!(a.index, 0);
    }

    #[test]
    fn attachment_bytes_are_transfer_decoded() {
        let (bytes, ctype, name) = attachment_bytes(MSG, 0).expect("attachment 0");
        assert_eq!(bytes, vec![0x50, 0x4B, 0x03, 0x04, 0x0A]); // "PK\x03\x04\n"
        assert_eq!(ctype, "application/zip");
        assert_eq!(name, "report.zip");
        assert!(attachment_bytes(MSG, 1).is_none(), "no second attachment");
    }
}
