//! `EmailSubmission/set` (RFC 8621 §7): sending a composed message.
//!
//! The draft is sent through the SMTP **trusted internal submission listener**
//! using the shared production SMTP client (`ficina-smtp-client`) — the same
//! client the delivery path uses — so the message is DKIM-signed, queued, and
//! delivered by the existing outbound path. See
//! `docs/design/email-submission.md`.
//!
//! Send-as is enforced here: `MAIL FROM` must be the authenticated user's own
//! canonical address or a registered alias (`forbiddenFrom` otherwise), so a
//! bearer token cannot send as another identity. On success the message is
//! un-drafted, marked seen, and filed into Sent.

use std::collections::HashSet;

use ficina_smtp_client::client::{OutboundSession, RcptOutcome};
use ficina_store::{MAX_PAGE, MessageId, Page};
use serde_json::{Map, Value, json};

use crate::state::{Account, AppState};

/// Maximum recipients accepted in one submission (anti-abuse; a per-user send
/// rate quota is a tracked follow-up — see docs/design/security-audit-followups.md).
const MAX_RECIPIENTS: usize = 100;

/// `EmailSubmission/set`. Only `create` is meaningful (a submission is a
/// transient action); `update`/`destroy` are accepted as no-ops.
pub async fn set(account: &Account, args: &Value, state: &AppState) -> Result<Value, Value> {
    crate::api::check_account(args, account)?;
    let st = account.acc.state().await.unwrap_or_else(|_| String::new());

    let mut created = Map::new();
    let mut not_created = Map::new();
    if let Some(creates) = args.get("create").and_then(Value::as_object) {
        for (cid, props) in creates {
            match create_one(account, props, state).await {
                Ok(email_id) => {
                    post_send(account, &MessageId::new(&email_id)).await;
                    created.insert(
                        cid.clone(),
                        json!({ "id": format!("s{email_id}"), "emailId": email_id }),
                    );
                }
                Err(e) => {
                    not_created.insert(cid.clone(), e);
                }
            }
        }
    }

    Ok(json!({
        "accountId": account.account_id(),
        "oldState": st, "newState": st,
        "created": created, "notCreated": not_created,
        "updated": {}, "notUpdated": {}, "destroyed": [], "notDestroyed": {}
    }))
}

async fn create_one(account: &Account, props: &Value, state: &AppState) -> Result<String, Value> {
    // 1. The draft to send.
    let Some(email_id) = props.get("emailId").and_then(Value::as_str) else {
        return Err(set_err("invalidProperties", "emailId is required"));
    };
    let mid = MessageId::new(email_id);

    // 2. Its bytes. The account door scopes this: a foreign or absent id is a
    // clean notFound, never another tenant's message.
    let bytes = account
        .acc
        .message_bytes(&mid)
        .await
        .map_err(|_| set_err("notFound", "emailId not found"))?;

    // 3. The user's valid send-from addresses (canonical + aliases).
    let ts = state.store.for_tenant(account.tenant.clone());
    let canonical = ts
        .email_of(&account.user)
        .await
        .map_err(|_| set_err("forbiddenToSend", "sender lookup failed"))?
        .ok_or_else(|| set_err("forbiddenFrom", "no address for this account"))?;
    let mut valid: HashSet<String> = HashSet::new();
    valid.insert(canonical.to_lowercase());
    if let Ok(aliases) = ts.aliases_of(&account.user).await {
        for a in aliases {
            valid.insert(a.to_lowercase());
        }
    }

    // Only a draft is sendable (a received/sent message is not re-sendable).
    let keywords = account
        .acc
        .keywords(&mid)
        .await
        .map_err(|_| set_err("forbiddenToSend", "could not read the message"))?;
    if !keywords.iter().any(|k| k == "$draft") {
        return Err(set_err("forbiddenToSend", "only a draft can be submitted"));
    }

    // The visible `From:` header — not only the SMTP envelope — MUST be an
    // address this account owns. Otherwise a bearer token could send a
    // DKIM-signed message with a forged author (intra-domain impersonation),
    // since the outbound path signs with our domain and does not rewrite From.
    let from_header = extract_from_addr(bytes.as_ref())
        .ok_or_else(|| set_err("forbiddenFrom", "the message has no From address"))?;
    if !valid.contains(&from_header) {
        return Err(set_err(
            "forbiddenFrom",
            "the message From is not an address of this account",
        ));
    }

    // 4. Envelope. mailFrom defaults to the canonical address and MUST be one
    // the account owns; rcptTo is taken from the envelope.
    let env = props.get("envelope");
    let mail_from = env
        .and_then(|e| e.get("mailFrom"))
        .and_then(|m| m.get("email"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| canonical.clone());
    if !valid_addr(&mail_from) || !valid.contains(&mail_from.to_lowercase()) {
        return Err(set_err(
            "forbiddenFrom",
            "mailFrom is not an address of this account",
        ));
    }
    let rcpts: Vec<String> = env
        .and_then(|e| e.get("rcptTo"))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|r| r.get("email").and_then(Value::as_str))
                .filter(|e| valid_addr(e))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    if rcpts.is_empty() {
        return Err(set_err(
            "noRecipients",
            "the envelope has no valid recipients",
        ));
    }
    if rcpts.len() > MAX_RECIPIENTS {
        return Err(set_err(
            "tooManyRecipients",
            "too many recipients for one message",
        ));
    }

    // 5. Submit through the trusted internal listener (DKIM-signed + queued
    // there). Failures are logged server-side without recipient/body detail.
    let Some(addr) = state.submission_addr.as_deref() else {
        tracing::error!("EmailSubmission/set: no submission listener configured");
        return Err(set_err("forbiddenToSend", "sending is not available"));
    };
    submit(addr, &mail_from, &rcpts, bytes.as_ref())
        .await
        .map_err(|reason| {
            tracing::error!(reason = %reason, "EmailSubmission/set: submission failed");
            set_err("forbiddenToSend", "the message could not be sent")
        })?;

    Ok(email_id.to_owned())
}

/// One SMTP transaction to the internal listener via the shared client.
async fn submit(
    addr: &str,
    mail_from: &str,
    rcpts: &[String],
    message: &[u8],
) -> Result<(), String> {
    let sockaddr = tokio::net::lookup_host(addr)
        .await
        .map_err(|e| format!("resolve: {e}"))?
        .next()
        .ok_or_else(|| "no address for submission host".to_owned())?;
    let mut session = OutboundSession::connect_addr(sockaddr, "ficina-jmap")
        .await
        .map_err(|e| format!("connect: {e}"))?;
    let outcomes = session
        .deliver(Some(mail_from), rcpts, message)
        .await
        .map_err(|e| format!("transaction: {e}"))?;
    session.quit().await;
    // The message reaches DATA only once any recipient is accepted, so it is
    // spooled the moment one is Delivered. Treat "any accepted" as success:
    // erroring after a partial acceptance would make a client retry and
    // double-send to the accepted recipients. Only a zero-acceptance result
    // (the relay took nothing) is a send failure. Addresses are never logged
    // (Law 1) — only the outcome class.
    if outcomes.iter().any(|o| matches!(o, RcptOutcome::Delivered)) {
        Ok(())
    } else {
        Err("the relay accepted no recipients".into())
    }
}

/// After a successful send: clear `$draft`, mark `$seen`, and file into Sent
/// (removing it from Drafts). Best-effort — the mail is already sent, so a
/// filing hiccup is logged, never surfaced as a send failure.
async fn post_send(account: &Account, mid: &MessageId) {
    if let Err(error) = account.acc.set_keyword(mid, "$draft", false).await {
        tracing::warn!(%error, "post-send: could not clear $draft");
    }
    if let Err(error) = account.acc.set_keyword(mid, "$seen", true).await {
        tracing::warn!(%error, "post-send: could not set $seen");
    }
    let boxes = match account.acc.mailboxes(Page::first(MAX_PAGE)).await {
        Ok(boxes) => boxes,
        Err(error) => {
            tracing::warn!(%error, "post-send: mailbox list failed");
            return;
        }
    };
    let Some(sent) = boxes.iter().find(|m| m.role.as_deref() == Some("sent")) else {
        return; // no Sent mailbox: leave the message where it is
    };
    if let Err(error) = account.acc.add_to_mailbox(mid, &sent.id).await {
        tracing::warn!(%error, "post-send: could not file to Sent");
        return;
    }
    for drafts in boxes.iter().filter(|m| m.role.as_deref() == Some("drafts")) {
        if let Err(error) = account.acc.remove_from_mailbox(mid, &drafts.id).await {
            tracing::warn!(%error, "post-send: could not remove from Drafts");
        }
    }
}

fn set_err(kind: &str, description: &str) -> Value {
    json!({ "type": kind, "description": description })
}

/// A safe addr-spec for an SMTP command: non-empty, has `@`, and contains no
/// whitespace, control chars, or angle brackets (no SMTP-command injection).
fn valid_addr(addr: &str) -> bool {
    !addr.is_empty()
        && addr.len() <= 320
        && addr.contains('@')
        && addr
            .bytes()
            .all(|b| b > 0x20 && b != b'<' && b != b'>' && b != 0x7f)
}

/// The lowercase addr-spec of a message's `From:` header (the address inside
/// the last `<…>`, else the trimmed value), honoring folded continuation
/// lines. `None` if absent or without an `@`. Used to bind the *visible*
/// author to the authenticated account (defence against From spoofing).
fn extract_from_addr(msg: &[u8]) -> Option<String> {
    let end = msg
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or(msg.len());
    let text = String::from_utf8_lossy(&msg[..end]);
    let mut lines = text.split("\r\n").peekable();
    let mut value: Option<String> = None;
    while let Some(line) = lines.next() {
        if line.len() >= 5
            && line
                .get(..5)
                .is_some_and(|p| p.eq_ignore_ascii_case("from:"))
        {
            let mut v = line[5..].to_string();
            while let Some(next) = lines.peek() {
                if next.starts_with(' ') || next.starts_with('\t') {
                    v.push(' ');
                    v.push_str(next.trim_start());
                    lines.next();
                } else {
                    break;
                }
            }
            value = Some(v);
            break;
        }
    }
    let v = value?;
    let addr = match (v.rfind('<'), v.rfind('>')) {
        (Some(lt), Some(gt)) if lt < gt => v[lt + 1..gt].trim().to_string(),
        _ => v.trim().to_string(),
    };
    if addr.contains('@') {
        Some(addr.to_lowercase())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_from_addr, valid_addr};

    #[test]
    fn accepts_a_normal_address() {
        assert!(valid_addr("alice@example.eu"));
    }

    #[test]
    fn rejects_injection_and_malformed() {
        assert!(!valid_addr("a@x.eu\r\nRCPT TO:<evil@x>"));
        assert!(!valid_addr("a@x.eu evil@x"));
        assert!(!valid_addr("<a@x.eu>"));
        assert!(!valid_addr("noatsign"));
        assert!(!valid_addr(""));
    }

    #[test]
    fn extracts_from_addr_forms() {
        assert_eq!(
            extract_from_addr(b"From: \"Disan\" <Disan@Namel3ss.com>\r\nTo: x@y\r\n\r\nbody"),
            Some("disan@namel3ss.com".to_owned())
        );
        assert_eq!(
            extract_from_addr(b"Subject: hi\r\nfrom: bare@example.eu\r\n\r\nbody"),
            Some("bare@example.eu".to_owned())
        );
        assert_eq!(extract_from_addr(b"To: x@y\r\n\r\nbody"), None);
    }
}
