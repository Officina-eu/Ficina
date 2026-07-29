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
        ChatMessage { role: "system".to_owned(), content: IMPROVE_SYSTEM.to_owned() },
        ChatMessage { role: "user".to_owned(), content: draft.to_owned() },
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

/// Improve an email draft via the configured backend. User-invoked only.
///
/// # Errors
/// [`InferenceError`] variants for disabled/unconfigured/unreachable/backend/
/// empty — all safe to surface (no message content leaks).
pub async fn improve(config: &AiConfig, draft: &str) -> Result<String, InferenceError> {
    if !config.enabled {
        return Err(InferenceError::Disabled);
    }
    if config.base_url.trim().is_empty() || config.model.trim().is_empty() {
        return Err(InferenceError::NotConfigured);
    }

    let url = format!("{}/v1/chat/completions", config.base_url.trim_end_matches('/'));
    let messages = improve_messages(draft);
    let body = ChatRequest {
        model: config.model.trim(),
        messages: &messages,
        temperature: 0.3,
        stream: false,
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|_| InferenceError::Transport)?;
    let mut request = client.post(&url).json(&body);
    if let Some(key) = &config.api_key
        && !key.trim().is_empty()
    {
        request = request.bearer_auth(key.trim());
    }

    let response = request.send().await.map_err(|_| InferenceError::Transport)?;
    let status = response.status();
    if !status.is_success() {
        return Err(InferenceError::Backend(status.as_u16()));
    }
    let text = response.text().await.map_err(|_| InferenceError::Transport)?;
    parse_completion(&text)
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
    fn parse_completion_extracts_and_trims_content() {
        let body = r#"{"choices":[{"message":{"role":"assistant","content":"  Improved text.  "}}]}"#;
        assert_eq!(parse_completion(body).unwrap(), "Improved text.");
    }

    #[test]
    fn parse_completion_rejects_empty_and_garbage() {
        assert!(matches!(parse_completion("{}"), Err(InferenceError::Empty)));
        assert!(matches!(parse_completion("not json"), Err(InferenceError::Empty)));
        let no_text = r#"{"choices":[{"message":{"role":"assistant","content":"   "}}]}"#;
        assert!(matches!(parse_completion(no_text), Err(InferenceError::Empty)));
    }

    #[tokio::test]
    async fn disabled_config_short_circuits_without_network() {
        let out = improve(&cfg(false, "http://localhost:11434", "llama3.2"), "hi").await;
        assert!(matches!(out, Err(InferenceError::Disabled)));
    }

    #[tokio::test]
    async fn enabled_but_unconfigured_is_not_configured() {
        let out = improve(&cfg(true, "", "llama3.2"), "hi").await;
        assert!(matches!(out, Err(InferenceError::NotConfigured)));
        let out = improve(&cfg(true, "http://localhost:11434", "  "), "hi").await;
        assert!(matches!(out, Err(InferenceError::NotConfigured)));
    }
}
