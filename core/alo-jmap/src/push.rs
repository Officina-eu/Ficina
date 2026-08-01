//! Push (RFC 8620 §7.3): a per-tenant broadcast fan-out and the
//! `text/event-stream` EventSource endpoint. Each connection is
//! authenticated to one tenant and subscribes to **that tenant's**
//! channel only, so a tenant's stream is structurally silent about other
//! tenants (an isolation surface — tested).

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use serde_json::{Map, json};
use tokio::sync::broadcast;

use crate::error::Problem;
use crate::state::{AppState, authenticate};

/// A state-change notification for one account.
#[derive(Debug, Clone)]
pub struct StateChangeMsg {
    /// The account (user id) whose data changed.
    pub account_id: String,
    /// The JMAP types that changed (`Mailbox`/`Email`/`Thread`).
    pub types: Vec<&'static str>,
    /// The new opaque state string (shared tenant modseq).
    pub state: String,
}

/// A per-tenant broadcast hub. Channels are created lazily on first
/// subscribe; publishing to a tenant with no subscribers is a no-op.
#[derive(Clone, Default)]
pub struct PushHub {
    inner: Arc<Mutex<HashMap<String, broadcast::Sender<StateChangeMsg>>>>,
}

impl PushHub {
    /// A fresh hub.
    pub fn new() -> Self {
        Self::default()
    }

    fn sender(&self, tenant: &str) -> broadcast::Sender<StateChangeMsg> {
        let mut map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        map.entry(tenant.to_owned())
            .or_insert_with(|| broadcast::channel(256).0)
            .clone()
    }

    /// Publishes a change to a tenant's channel (no-op if nobody listens).
    pub fn publish(&self, tenant: &str, msg: StateChangeMsg) {
        let map = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(tx) = map.get(tenant) {
            let _ = tx.send(msg);
        }
    }

    /// Subscribes to a tenant's channel.
    pub fn subscribe(&self, tenant: &str) -> broadcast::Receiver<StateChangeMsg> {
        self.sender(tenant).subscribe()
    }
}

/// `GET {eventSourceUrl}` — a `text/event-stream` emitting `StateChange`
/// events for this account, with keep-alive heartbeats.
pub async fn event_source(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let account = authenticate(&state, &headers).await?;
    // The accounts this connection listens for: the user's own, plus any shared
    // mailboxes they were delegated (ADR 0017), so a change made by another
    // delegate reaches this client live. Computed at connect time — a grant
    // added mid-connection takes effect on the next reconnect.
    let mut account_ids: std::collections::HashSet<String> =
        std::collections::HashSet::from([account.account_id().to_owned()]);
    if let Ok(delegations) = state
        .store
        .for_tenant(account.tenant.clone())
        .delegations_for(&account.user)
        .await
    {
        for (owner_id, _email, _can_write, _send_mode) in delegations {
            account_ids.insert(owner_id);
        }
    }
    let mut rx = state.push.subscribe(account.tenant.as_str());

    let stream = futures::stream::unfold(
        (rx.resubscribe(), account_ids),
        move |(mut rx, account_ids)| async move {
            loop {
                match rx.recv().await {
                    Ok(msg) if account_ids.contains(&msg.account_id) => {
                        let event = Event::default()
                            .event("state")
                            .id(msg.state.clone())
                            .data(state_change_json(&msg).to_string());
                        return Some((Ok::<_, Infallible>(event), (rx, account_ids)));
                    }
                    // Another account in the same tenant, or a lag skip.
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
        },
    );
    let _ = &mut rx; // keep the original subscription's lifetime tidy

    Ok(Sse::new(Box::pin(stream))
        .keep_alive(KeepAlive::default())
        .into_response())
}

/// The RFC 8620 §7.1 `StateChange` object for one account.
fn state_change_json(msg: &StateChangeMsg) -> serde_json::Value {
    let mut types = Map::new();
    for t in &msg.types {
        types.insert((*t).to_owned(), json!(msg.state));
    }
    let mut changed = Map::new();
    changed.insert(msg.account_id.clone(), serde_json::Value::Object(types));
    json!({ "@type": "StateChange", "changed": changed })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use tokio::sync::broadcast::error::TryRecvError;

    #[tokio::test]
    async fn push_is_per_tenant_isolated() {
        let hub = PushHub::new();
        let mut a = hub.subscribe("tenant-a");
        let mut b = hub.subscribe("tenant-b");
        hub.publish(
            "tenant-a",
            StateChangeMsg {
                account_id: "user-a".to_owned(),
                types: vec!["Email"],
                state: "7".to_owned(),
            },
        );
        // Tenant A's stream receives it; tenant B's is silent.
        let got = a.try_recv().unwrap();
        assert_eq!(got.account_id, "user-a");
        assert!(matches!(b.try_recv(), Err(TryRecvError::Empty)));
    }
}
