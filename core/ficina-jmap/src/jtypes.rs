//! JMAP JSON representations of store entities (RFC 8621). Keeps the
//! wire shapes — the public contract — in one place.

use ficina_store::{Mailbox, Message};
use serde_json::{Map, Value, json};
use time::OffsetDateTime;

/// A JMAP `UTCDate`: `YYYY-MM-DDTHH:MM:SSZ`.
pub fn utc_date(dt: OffsetDateTime) -> String {
    let dt = dt.to_offset(time::UtcOffset::UTC);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        dt.year(),
        dt.month() as u8,
        dt.day(),
        dt.hour(),
        dt.minute(),
        dt.second()
    )
}

/// A JMAP `Mailbox` (RFC 8621 §2). Counters come straight from the
/// store's transactional totals — never recomputed here. Thread counts
/// are approximated by the email counts for now (documented in
/// `docs/interop.md`).
pub fn mailbox_json(m: &Mailbox) -> Value {
    json!({
        "id": m.id.as_str(),
        "name": m.name,
        "parentId": m.parent_id.as_ref().map(|p| p.as_str()),
        "role": m.role,
        "sortOrder": 0,
        "totalEmails": m.total_messages,
        "unreadEmails": m.unread_messages,
        "totalThreads": m.total_messages,
        "unreadThreads": m.unread_messages,
        "myRights": {
            "mayReadItems": true, "mayAddItems": true, "mayRemoveItems": true,
            "maySetSeen": true, "maySetKeywords": true, "mayCreateChild": true,
            "mayRename": true, "mayDelete": true, "maySubmit": true
        },
        "isSubscribed": true
    })
}

/// One JMAP `EmailAddress` parsed from a raw header value (best effort):
/// the address inside `<...>` (or the whole string), plus any display
/// name before it.
fn address_list(raw: &str) -> Value {
    if raw.trim().is_empty() {
        return json!([]);
    }
    let mut out = Vec::new();
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (name, email) = match (part.find('<'), part.find('>')) {
            (Some(lt), Some(gt)) if gt > lt => {
                let email = part[lt + 1..gt].trim().to_owned();
                let name = part[..lt].trim().trim_matches('"').to_owned();
                (if name.is_empty() { None } else { Some(name) }, email)
            }
            _ => (None, part.to_owned()),
        };
        out.push(json!({ "name": name, "email": email }));
    }
    json!(out)
}

/// Builds a full JMAP `Email` object from the stored metadata plus the
/// resolved mailbox ids, keywords, and (optional) body value.
///
/// `body` is the extracted text body; `truncated` marks whether it was
/// cut to the `bodyValues` ceiling. `fetch_body` controls whether
/// `bodyValues` is populated (the client asked for it).
#[allow(clippy::too_many_arguments)]
pub fn email_json(
    m: &Message,
    mailbox_ids: &[String],
    keywords: &[String],
    body: Option<(&str, bool)>,
    has_attachment: bool,
) -> Value {
    let mut mailboxes = Map::new();
    for id in mailbox_ids {
        mailboxes.insert(id.clone(), json!(true));
    }
    let mut kw = Map::new();
    for k in keywords {
        kw.insert(k.clone(), json!(true));
    }

    let preview: String = body
        .map(|(b, _)| b.chars().take(256).collect())
        .unwrap_or_default();

    let mut email = json!({
        "id": m.id.as_str(),
        "blobId": m.blob_id.as_str(),
        "threadId": m.thread_id.as_str(),
        "mailboxIds": Value::Object(mailboxes),
        "keywords": Value::Object(kw),
        "size": m.size,
        "receivedAt": utc_date(m.received_at),
        "sentAt": m.sent_at.map(utc_date),
        "subject": m.subject,
        "from": address_list(&m.from_addr),
        "to": address_list(&m.to_addrs),
        "preview": preview,
        "hasAttachment": has_attachment,
        "messageId": m.message_id_hdr.as_ref().map(|v| vec![v.clone()]),
        "textBody": [ { "partId": "1", "type": "text/plain", "blobId": m.blob_id.as_str() } ],
        // Ficina exposes the parsed auth verdict as a non-standard
        // property so clients can render a trust banner without a header
        // fetch (additive, `ficina:` namespaced).
        "ficina:authentication": {
            "spf": m.auth_spf, "dkim": m.auth_dkim, "dmarc": m.auth_dmarc
        }
    });

    if let Some((value, truncated)) = body {
        email["bodyValues"] = json!({
            "1": { "value": value, "isEncodingProblem": false, "isTruncated": truncated }
        });
    }
    email
}

/// A JMAP `Thread` object (§3): id + ordered email ids.
pub fn thread_json(thread_id: &str, email_ids: &[String]) -> Value {
    json!({ "id": thread_id, "emailIds": email_ids })
}
