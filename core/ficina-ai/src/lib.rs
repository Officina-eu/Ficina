//! Ficina AI inference layer (ADR 0011).
//!
//! Model-agnostic by construction: it speaks one wire contract — the
//! OpenAI-compatible **Chat Completions** API (`{base}/v1/chat/completions`) —
//! which Ollama, vLLM, and every hosted provider we care about implement. The
//! backend is *configured, never bundled*: an operator supplies a base URL, a
//! model, and (optionally) an API key, per tenant.
//!
//! Privacy (constitution law #1): the only thing sent to the backend is the
//! text the user asked us to act on. Prompts and completions are **never
//! logged**, and errors carry status codes only — never response bodies.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Per-tenant backend configuration (admin-set, ADR 0011).
#[derive(Debug, Clone)]
pub struct AiConfig {
    /// Base URL of an OpenAI-compatible endpoint, e.g. `http://localhost:11434`
    /// (Ollama) or `https://api.mistral.ai`.
    pub base_url: String,
    /// The model name to request, e.g. `llama3.2` or `mistral-small-latest`.
    pub model: String,
    /// Optional bearer key for hosted providers; `None`/empty for local Ollama.
    pub api_key: Option<String>,
    /// Whether AI is enabled for this tenant. When false, calls fail with
    /// [`InferenceError::Disabled`] and callers hide the feature.
    pub enabled: bool,
}

/// Why an inference call did not produce text. Deliberately coarse — it never
/// carries a backend response body (law #1).
#[derive(Debug, thiserror::Error)]
pub enum InferenceError {
    /// AI is switched off for this tenant.
    #[error("ai disabled for tenant")]
    Disabled,
    /// AI is on but no usable endpoint/model is configured.
    #[error("ai not configured")]
    NotConfigured,
    /// The backend answered but with no usable content.
    #[error("empty completion")]
    Empty,
    /// The backend returned a non-success status (code only, no body).
    #[error("inference backend status {0}")]
    Backend(u16),
    /// The backend could not be reached (DNS/TLS/timeout).
    #[error("inference backend unreachable")]
    Transport,
}

/// One chat message in the OpenAI Chat Completions shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    temperature: f32,
    stream: bool,
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: RespMessage,
}

#[derive(Deserialize)]
struct RespMessage {
    #[serde(default)]
    content: String,
}

const IMPROVE_SYSTEM: &str = "You are an editor for email drafts. Improve the \
draft you are given: fix grammar, spelling, clarity, and tone while preserving \
its original meaning and the language it is written in. Return only the \
improved text — no preamble, explanation, or quotation.";

/// The chat messages for an "improve this draft" request. Pure and exported so
/// the prompt is testable without a backend.
pub fn improve_messages(draft: &str) -> Vec<ChatMessage> {
    vec![
        ChatMessage {
            role: "system".to_owned(),
            content: IMPROVE_SYSTEM.to_owned(),
        },
        ChatMessage {
            role: "user".to_owned(),
            content: draft.to_owned(),
        },
    ]
}

/// Extract the assistant's text from an OpenAI-compatible response body. Pure
/// and exported for testing.
///
/// # Errors
/// [`InferenceError::Empty`] if the body does not parse or yields no text.
pub fn parse_completion(body: &str) -> Result<String, InferenceError> {
    let resp: ChatResponse = serde_json::from_str(body).map_err(|_| InferenceError::Empty)?;
    let text = resp
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .unwrap_or_default();
    let text = text.trim().to_owned();
    if text.is_empty() {
        return Err(InferenceError::Empty);
    }
    Ok(text)
}

/// The largest inference response we will buffer. A hostile or broken backend
/// must not be able to exhaust memory (law #2, full path incl. error paths);
/// 4 MiB dwarfs any legitimate improved email draft.
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

/// Whether a resolved IP must be refused under the restricted egress policy:
/// loopback, link-local (which includes the `169.254.169.254` cloud-metadata
/// endpoint), private, unique-local, unspecified, or multicast/reserved —
/// anything that could reach the host itself, a co-tenant, or internal
/// infrastructure (ADR 0012 SSRF guard).
fn is_blocked_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.octets()[0] == 0
                || v4.octets()[0] >= 224 // multicast + reserved
        }
        std::net::IpAddr::V6(v6) => {
            if v6.is_loopback() || v6.is_unspecified() || v6.is_multicast() {
                return true;
            }
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_blocked_ip(std::net::IpAddr::V4(mapped));
            }
            let seg0 = v6.segments()[0];
            (seg0 & 0xfe00) == 0xfc00 // unique-local fc00::/7
                || (seg0 & 0xffc0) == 0xfe80 // link-local fe80::/10
        }
    }
}

/// Parse `(is_https, host, port)` from a URL without a URL-crate dependency.
fn split_authority(url: &str) -> Option<(bool, String, u16)> {
    let (scheme, rest) = url.split_once("://")?;
    let https = scheme.eq_ignore_ascii_case("https");
    let authority = rest.split(['/', '?', '#']).next()?;
    let authority = authority.rsplit('@').next()?; // drop any userinfo
    let (host, port) = match authority.rsplit_once(':') {
        // host:port — but not an unbracketed IPv6 literal (has multiple colons).
        Some((h, p)) if !h.is_empty() && !h.contains(':') => (h.to_owned(), p.parse().ok()?),
        _ => (authority.to_owned(), if https { 443 } else { 80 }),
    };
    if host.is_empty() {
        return None;
    }
    Some((https, host, port))
}

/// Build an HTTP client for a backend at `url`, enforcing the egress policy.
/// Default `open` mode (self-hosted — the model runs on localhost or the
/// private LAN) allows any host. `restricted` mode (`FICINA_AI_EGRESS=restricted`,
/// set on shared/hosted deployments) requires https and refuses any host that
/// resolves to a loopback/link-local/private/ULA address, then **pins** the
/// vetted address so a DNS rebind between check and connect cannot slip through.
/// Every rejection returns the same `Transport` error as a genuinely
/// unreachable host — no oracle that reveals what is internally reachable.
async fn build_client(url: &str, timeout: Duration) -> Result<reqwest::Client, InferenceError> {
    let restricted = std::env::var("FICINA_AI_EGRESS")
        .map(|v| v.trim().eq_ignore_ascii_case("restricted"))
        .unwrap_or(false);
    let mut builder = reqwest::Client::builder().timeout(timeout);
    if restricted {
        let (https, host, port) = split_authority(url).ok_or(InferenceError::Transport)?;
        if !https {
            return Err(InferenceError::Transport);
        }
        let addrs: Vec<std::net::SocketAddr> = tokio::net::lookup_host((host.as_str(), port))
            .await
            .map_err(|_| InferenceError::Transport)?
            .collect();
        if addrs.is_empty() || addrs.iter().any(|a| is_blocked_ip(a.ip())) {
            return Err(InferenceError::Transport);
        }
        if let Some(first) = addrs.first() {
            builder = builder.resolve(&host, *first);
        }
    }
    builder.build().map_err(|_| InferenceError::Transport)
}

/// Build `{base}/v1/{path}`, tolerating a base that already ends in `/v1` — the
/// form hosted providers print in their docs (`https://api.openai.com/v1`) — or
/// carries a trailing slash. Without this, such a base doubles the segment into
/// `/v1/v1/...` and every request 404s.
fn endpoint(base_url: &str, path: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    let base = base.strip_suffix("/v1").unwrap_or(base);
    format!("{base}/v1/{path}")
}

/// Read a response body, refusing anything larger than [`MAX_RESPONSE_BYTES`].
/// Streams chunk-by-chunk so an over-large (or lying `Content-Length`) backend
/// is rejected without first buffering it whole.
async fn read_body_capped(mut response: reqwest::Response) -> Result<String, InferenceError> {
    let mut buf = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| InferenceError::Transport)?
    {
        if buf.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(InferenceError::Empty);
        }
        buf.extend_from_slice(&chunk);
    }
    String::from_utf8(buf).map_err(|_| InferenceError::Empty)
}

/// The system prompt for summarizing an email thread (ADR 0011).
const SUMMARIZE_SYSTEM: &str = "You summarize an email thread for its recipient. \
In one or two short sentences, say what the thread is about and any action or \
decision the recipient needs to make, in the thread's own language. Be concrete. \
Return only the summary — no preamble, heading, or quotation.";

/// The chat messages for a "summarize this thread" request. Pure and exported
/// so the prompt is testable without a backend.
pub fn summarize_messages(thread: &str) -> Vec<ChatMessage> {
    vec![
        ChatMessage {
            role: "system".to_owned(),
            content: SUMMARIZE_SYSTEM.to_owned(),
        },
        ChatMessage {
            role: "user".to_owned(),
            content: thread.to_owned(),
        },
    ]
}

/// One chat-completions round-trip to the configured backend, returning the
/// assistant's text. Shared by [`improve`] and [`summarize`]; enforces the
/// enabled/configured gates and the egress policy, and never logs content.
async fn chat(
    config: &AiConfig,
    messages: &[ChatMessage],
    temperature: f32,
) -> Result<String, InferenceError> {
    if !config.enabled {
        return Err(InferenceError::Disabled);
    }
    if config.base_url.trim().is_empty() || config.model.trim().is_empty() {
        return Err(InferenceError::NotConfigured);
    }
    let url = endpoint(&config.base_url, "chat/completions");
    let body = ChatRequest {
        model: config.model.trim(),
        messages,
        temperature,
        stream: false,
    };
    let client = build_client(&url, Duration::from_secs(60)).await?;
    let mut request = client.post(&url).json(&body);
    if let Some(key) = &config.api_key
        && !key.trim().is_empty()
    {
        request = request.bearer_auth(key.trim());
    }
    let response = request
        .send()
        .await
        .map_err(|_| InferenceError::Transport)?;
    let status = response.status();
    if !status.is_success() {
        return Err(InferenceError::Backend(status.as_u16()));
    }
    let text = read_body_capped(response).await?;
    parse_completion(&text)
}

/// Improve an email draft via the configured backend. User-invoked only.
///
/// # Errors
/// [`InferenceError`] variants for disabled/unconfigured/unreachable/backend/
/// empty — all safe to surface (no message content leaks).
pub async fn improve(config: &AiConfig, draft: &str) -> Result<String, InferenceError> {
    chat(config, &improve_messages(draft), 0.3).await
}

/// Summarize an email thread via the configured backend (ADR 0011). The reading
/// pane calls this when a conversation opens.
///
/// # Errors
/// [`InferenceError`] variants for disabled/unconfigured/unreachable/backend/
/// empty — all safe to surface (no message content leaks).
pub async fn summarize(config: &AiConfig, thread: &str) -> Result<String, InferenceError> {
    chat(config, &summarize_messages(thread), 0.2).await
}

/// A lightweight connectivity check for the admin "Test connection" action:
/// `GET {base}/v1/models`. Returns the number of models the endpoint reports.
/// Unlike [`improve`] it does not gate on `enabled` — the admin is testing a
/// config that may not be saved yet.
///
/// # Errors
/// [`InferenceError::NotConfigured`] for an empty base URL; `Backend`/`Transport`
/// on an HTTP failure; `Empty` if the response is not the expected shape.
pub async fn check(base_url: &str, api_key: Option<&str>) -> Result<usize, InferenceError> {
    if base_url.trim().is_empty() {
        return Err(InferenceError::NotConfigured);
    }
    let url = endpoint(base_url, "models");
    let client = build_client(&url, Duration::from_secs(20)).await?;
    let mut request = client.get(&url);
    if let Some(key) = api_key
        && !key.trim().is_empty()
    {
        request = request.bearer_auth(key.trim());
    }
    let response = request
        .send()
        .await
        .map_err(|_| InferenceError::Transport)?;
    if !response.status().is_success() {
        return Err(InferenceError::Backend(response.status().as_u16()));
    }
    let body = read_body_capped(response).await?;
    let parsed: serde_json::Value =
        serde_json::from_str(&body).map_err(|_| InferenceError::Empty)?;
    Ok(parsed
        .get("data")
        .and_then(|d| d.as_array())
        .map(Vec::len)
        .unwrap_or(0))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn cfg(enabled: bool, base: &str, model: &str) -> AiConfig {
        AiConfig {
            base_url: base.to_owned(),
            model: model.to_owned(),
            api_key: None,
            enabled,
        }
    }

    #[test]
    fn improve_messages_are_system_then_user() {
        let m = improve_messages("Hey, wanna meet tmrw?");
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].role, "system");
        assert_eq!(m[1].role, "user");
        assert_eq!(m[1].content, "Hey, wanna meet tmrw?");
    }

    #[test]
    fn blocked_ips_cover_the_ssrf_ranges() {
        use std::net::IpAddr;
        for ip in [
            "127.0.0.1",
            "169.254.169.254", // cloud metadata
            "10.0.0.5",
            "172.16.9.9",
            "192.168.1.1",
            "0.0.0.0",
            "::1",
            "fd00::1", // ULA
            "fe80::1", // link-local
        ] {
            assert!(
                is_blocked_ip(ip.parse::<IpAddr>().unwrap()),
                "should block {ip}"
            );
        }
        for ip in [
            "8.8.8.8",
            "1.1.1.1",
            "93.184.216.34",
            "2606:4700:4700::1111",
        ] {
            assert!(
                !is_blocked_ip(ip.parse::<IpAddr>().unwrap()),
                "should allow {ip}"
            );
        }
    }

    #[test]
    fn split_authority_parses_scheme_host_port() {
        assert_eq!(
            split_authority("https://api.openai.com/v1/chat/completions"),
            Some((true, "api.openai.com".to_owned(), 443))
        );
        assert_eq!(
            split_authority("http://localhost:11434/v1/models"),
            Some((false, "localhost".to_owned(), 11434))
        );
        assert_eq!(
            split_authority("https://mistral.example:8443/v1/models"),
            Some((true, "mistral.example".to_owned(), 8443))
        );
    }

    #[test]
    fn endpoint_appends_single_v1_regardless_of_base_shape() {
        // Ollama / custom: host root, no /v1.
        assert_eq!(
            endpoint("http://localhost:11434", "chat/completions"),
            "http://localhost:11434/v1/chat/completions"
        );
        // Hosted providers print the base *with* /v1 — must not double it.
        assert_eq!(
            endpoint("https://api.openai.com/v1", "chat/completions"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            endpoint("https://api.anthropic.com/v1", "models"),
            "https://api.anthropic.com/v1/models"
        );
        // Trailing slashes (with or without /v1) are tolerated too.
        assert_eq!(
            endpoint("https://api.openai.com/v1/", "models"),
            "https://api.openai.com/v1/models"
        );
        assert_eq!(
            endpoint("http://localhost:11434/", "models"),
            "http://localhost:11434/v1/models"
        );
    }

    #[test]
    fn parse_completion_extracts_and_trims_content() {
        let body =
            r#"{"choices":[{"message":{"role":"assistant","content":"  Improved text.  "}}]}"#;
        assert_eq!(parse_completion(body).unwrap(), "Improved text.");
    }

    #[test]
    fn parse_completion_rejects_empty_and_garbage() {
        assert!(matches!(parse_completion("{}"), Err(InferenceError::Empty)));
        assert!(matches!(
            parse_completion("not json"),
            Err(InferenceError::Empty)
        ));
        let no_text = r#"{"choices":[{"message":{"role":"assistant","content":"   "}}]}"#;
        assert!(matches!(
            parse_completion(no_text),
            Err(InferenceError::Empty)
        ));
    }

    #[tokio::test]
    async fn disabled_config_short_circuits_without_network() {
        let out = improve(&cfg(false, "http://localhost:11434", "llama3.2"), "hi").await;
        assert!(matches!(out, Err(InferenceError::Disabled)));
    }

    #[tokio::test]
    async fn check_requires_base_url() {
        assert!(matches!(
            check("", None).await,
            Err(InferenceError::NotConfigured)
        ));
    }

    #[tokio::test]
    async fn enabled_but_unconfigured_is_not_configured() {
        let out = improve(&cfg(true, "", "llama3.2"), "hi").await;
        assert!(matches!(out, Err(InferenceError::NotConfigured)));
        let out = improve(&cfg(true, "http://localhost:11434", "  "), "hi").await;
        assert!(matches!(out, Err(InferenceError::NotConfigured)));
    }
}
