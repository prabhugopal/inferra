use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role { System, User, Assistant }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: Role::User, content: content.into() }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationOptions {
    pub max_new_tokens: u32,
    pub temperature: f32,
    pub top_p: f32,
    pub seed: Option<u64>,
    /// Whether to ask the backend for per-token log-probabilities. Costs a
    /// full softmax over the vocabulary per decoded token, so turning it off
    /// can matter on slow (e.g. unquantized/CPU) backends.
    #[serde(default = "default_true")]
    pub want_logprobs: bool,
}

impl Default for GenerationOptions {
    fn default() -> Self {
        Self { max_new_tokens: 128, temperature: 0.0, top_p: 0.95, seed: Some(42), want_logprobs: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRequest {
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub options: GenerationOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    /// Prefill throughput, when the backend can separate it from decode. mistral.rs reports this.
    pub prompt_tokens_per_sec: Option<f32>,
    /// Decode throughput, when the backend can separate it from prefill. mistral.rs reports this.
    pub completion_tokens_per_sec: Option<f32>,
    /// Time spent processing the prompt (prefill), before the first generated token.
    pub prompt_time_sec: Option<f32>,
    /// Time spent generating completion tokens (decode).
    pub completion_time_sec: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResult {
    pub text: String,
    pub finish_reason: Option<String>,
    pub usage: Usage,
    pub elapsed: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub backend: String,
    pub model_id: String,
    pub endpoint: Option<String>,
}

#[derive(Debug, Error)]
pub enum InferenceError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("backend unavailable: {0}")]
    Unavailable(String),
    #[error("backend request failed: {0}")]
    Backend(String),
    #[error("backend returned an invalid response: {0}")]
    InvalidResponse(String),
}

pub type Result<T> = std::result::Result<T, InferenceError>;

/// A generated token's own probability, from the backend's logprobs — not an
/// alternative candidate (backends that expose alternatives only give raw
/// token IDs, not decoded text, so there's nothing honest to show for those).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenLogprob {
    pub token: String,
    pub logprob: f32,
}

/// One event from a streaming generation. `Done`/`Error` are terminal — no
/// further events follow either of them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GenerationEvent {
    Token { delta: String, logprob: Option<TokenLogprob> },
    Done { result: GenerationResult },
    Error { message: String },
}

#[async_trait]
pub trait InferenceBackend: Send + Sync {
    fn model_info(&self) -> ModelInfo;
    async fn generate(&self, request: GenerationRequest) -> Result<GenerationResult>;
    /// Streaming variant. The receiver yields `Token` events as they arrive,
    /// then exactly one terminal `Done` or `Error` event.
    async fn generate_stream(&self, request: GenerationRequest) -> Result<tokio::sync::mpsc::Receiver<GenerationEvent>>;
    /// Raw token IDs for `text`, using the loaded model's own tokenizer —
    /// no chat template applied, no special tokens added. Used to seed the
    /// embedding-space inspector from real text instead of a random sample.
    async fn tokenize(&self, text: &str) -> Result<Vec<u32>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_deterministic() {
        let options = GenerationOptions::default();
        assert_eq!(options.temperature, 0.0);
        assert_eq!(options.seed, Some(42));
    }
}
