//! UI-safe commands and DTOs. A future Tauri command should call this layer.

use inferra_app::InferenceService;
use inferra_core::{
    ChatMessage, GenerationEvent, GenerationOptions, GenerationRequest, GenerationResult,
    InferenceError, ModelInfo, TokenLogprob,
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Debug, Deserialize)]
pub struct GenerateCommand {
    pub prompt: String,
    #[serde(default)]
    pub options: GenerationOptions,
}

#[derive(Debug, Serialize)]
pub struct GenerateView {
    pub text: String,
    pub elapsed_ms: u128,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub prompt_tokens_per_sec: Option<f32>,
    pub completion_tokens_per_sec: Option<f32>,
    pub prompt_time_sec: Option<f32>,
    pub completion_time_sec: Option<f32>,
}

impl From<GenerationResult> for GenerateView {
    fn from(result: GenerationResult) -> Self {
        Self {
            text: result.text,
            elapsed_ms: result.elapsed.as_millis(),
            prompt_tokens: result.usage.prompt_tokens,
            completion_tokens: result.usage.completion_tokens,
            prompt_tokens_per_sec: result.usage.prompt_tokens_per_sec,
            completion_tokens_per_sec: result.usage.completion_tokens_per_sec,
            prompt_time_sec: result.usage.prompt_time_sec,
            completion_time_sec: result.usage.completion_time_sec,
        }
    }
}

/// Wire-facing streaming event — `Done` carries the same shape as the
/// non-streaming `GenerateView` so the frontend has one result shape either way.
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum GenerateStreamEvent {
    Token { delta: String, logprob: Option<TokenLogprob> },
    Done { view: GenerateView },
    Error { message: String },
}

#[derive(Clone)]
pub struct FrontendBridge { service: InferenceService }

impl FrontendBridge {
    pub fn new(service: InferenceService) -> Self { Self { service } }

    pub fn model_info(&self) -> ModelInfo { self.service.model_info() }

    pub async fn tokenize(&self, text: &str) -> Result<Vec<u32>, String> {
        self.service.tokenize(text).await.map_err(|error| error.to_string())
    }

    pub async fn generate(&self, command: GenerateCommand) -> Result<GenerateView, String> {
        if command.prompt.trim().is_empty() {
            return Err(InferenceError::InvalidRequest("prompt cannot be empty".into()).to_string());
        }
        let result = self.service.generate(GenerationRequest {
            messages: vec![ChatMessage::user(command.prompt)],
            options: command.options,
        }).await.map_err(|error| error.to_string())?;

        Ok(result.into())
    }

    /// Returns a receiver of UI-shaped events. The caller (a Tauri command)
    /// forwards each item to the frontend as it arrives.
    pub async fn generate_stream(&self, command: GenerateCommand) -> Result<mpsc::Receiver<GenerateStreamEvent>, String> {
        if command.prompt.trim().is_empty() {
            return Err(InferenceError::InvalidRequest("prompt cannot be empty".into()).to_string());
        }
        let mut core_rx = self.service.generate_stream(GenerationRequest {
            messages: vec![ChatMessage::user(command.prompt)],
            options: command.options,
        }).await.map_err(|error| error.to_string())?;

        let (tx, rx) = mpsc::channel(32);
        tokio::spawn(async move {
            while let Some(event) = core_rx.recv().await {
                let (mapped, is_terminal) = match event {
                    GenerationEvent::Token { delta, logprob } => (GenerateStreamEvent::Token { delta, logprob }, false),
                    GenerationEvent::Done { result } => (GenerateStreamEvent::Done { view: result.into() }, true),
                    GenerationEvent::Error { message } => (GenerateStreamEvent::Error { message }, true),
                };
                if tx.send(mapped).await.is_err() {
                    break;
                }
                if is_terminal {
                    break;
                }
            }
        });
        Ok(rx)
    }
}
