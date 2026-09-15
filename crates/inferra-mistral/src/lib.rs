//! Embedded adapter over the `mistralrs` Rust SDK.
//!
//! `mistral.rs` is loaded as an in-process library: the model lives in this
//! process' memory, with no `mistralrs-server` HTTP hop.

use async_trait::async_trait;
use either::Either;
use inferra_core::{
    GenerationEvent, GenerationRequest, GenerationResult, InferenceBackend, InferenceError,
    ModelInfo, Result, Role, TokenLogprob, Usage,
};
use mistralrs::{GgufModelBuilder, Model, RequestBuilder, Response, SamplingParams, TextMessageRole};
use mistralrs::core::ResponseLogprob;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;

pub struct MistralRsBackend {
    model: Arc<Model>,
    model_id: String,
}

impl MistralRsBackend {
    /// Loads a local GGUF file directly, without contacting any server.
    ///
    /// `tok_model_id` and `chat_template` are optional overrides for GGUF files
    /// that do not embed their own tokenizer/chat-template metadata.
    pub async fn load(
        model_dir: impl ToString,
        gguf_file: impl ToString,
        model_id: impl Into<String>,
        tok_model_id: Option<String>,
        chat_template: Option<String>,
    ) -> anyhow::Result<Self> {
        let mut builder = GgufModelBuilder::new(model_dir, vec![gguf_file]).with_logging();
        if let Some(id) = tok_model_id {
            builder = builder.with_tok_model_id(id);
        }
        if let Some(template) = chat_template {
            builder = builder.with_chat_template(template);
        }
        let model = builder.build().await?;
        Ok(Self { model: Arc::new(model), model_id: model_id.into() })
    }
}

fn build_chat_request(request: &GenerationRequest) -> RequestBuilder {
    let mut chat_request = RequestBuilder::new();
    for message in &request.messages {
        let role = match message.role {
            Role::System => TextMessageRole::System,
            Role::User => TextMessageRole::User,
            Role::Assistant => TextMessageRole::Assistant,
        };
        chat_request = chat_request.add_message(role, message.content.as_str());
    }

    let options = &request.options;
    // mistralrs' embedded SamplingParams has no seed field, so
    // GenerationOptions.seed is currently a no-op for this backend.
    chat_request
        .set_sampling(SamplingParams {
            temperature: Some(options.temperature as f64),
            top_p: Some(options.top_p as f64),
            max_len: Some(options.max_new_tokens as usize),
            ..SamplingParams::neutral()
        })
        // The chosen token's own probability, for the confidence report.
        // mistral.rs's alternative-candidate list only carries raw token
        // IDs (no decoded text), so there's nothing honest to show for
        // those — we only surface the token actually generated.
        .return_logprobs(options.want_logprobs)
}

/// `Some(value)` only if finite. A near-zero denominator (very fast prefill
/// or decode) can make mistral.rs report `inf`/`NaN` for a rate, and
/// `serde_json` refuses to encode either — silently breaking IPC/JSON
/// consumers downstream. `None` is the correct "not available" value here,
/// not a workaround.
fn finite(value: f32) -> Option<f32> {
    value.is_finite().then_some(value)
}

/// `Some` only if this is a mathematically valid log-probability: a genuine
/// log-probability (any base) is always <= 0, since it's the log of a value
/// in (0, 1]. The primary guard against mistral.rs 0.8's known-broken greedy
/// path is `is_greedy` below (a certainty, from its own routing threshold —
/// see the note there); this is defense-in-depth for any other path that
/// might misbehave the same way. (Fixed upstream as of mistralrs 0.9.3, but
/// we're pinned to 0.8 for now — see the note on `is_greedy`.)
fn valid_logprob(lp: ResponseLogprob) -> Option<TokenLogprob> {
    (lp.logprob.is_finite() && lp.logprob <= 0.0).then_some(TokenLogprob { token: lp.token, logprob: lp.logprob })
}

/// mistral.rs separates prefill from decode internally; surface that
/// breakdown instead of collapsing it into one end-to-end rate.
fn map_usage(usage: mistralrs::Usage) -> Usage {
    Usage {
        prompt_tokens: Some(usage.prompt_tokens as u64),
        completion_tokens: Some(usage.completion_tokens as u64),
        prompt_tokens_per_sec: finite(usage.avg_prompt_tok_per_sec),
        completion_tokens_per_sec: finite(usage.avg_compl_tok_per_sec),
        prompt_time_sec: finite(usage.total_prompt_time_sec),
        completion_time_sec: finite(usage.total_completion_time_sec),
    }
}

#[async_trait]
impl InferenceBackend for MistralRsBackend {
    fn model_info(&self) -> ModelInfo {
        ModelInfo {
            backend: "mistral.rs (embedded)".to_owned(),
            model_id: self.model_id.clone(),
            endpoint: None,
        }
    }

    async fn generate(&self, request: GenerationRequest) -> Result<GenerationResult> {
        if request.messages.is_empty() {
            return Err(InferenceError::InvalidRequest("at least one message is required".into()));
        }

        let chat_request = build_chat_request(&request);

        let started = Instant::now();
        let response = self.model.send_chat_request(chat_request).await
            .map_err(|error| InferenceError::Backend(error.to_string()))?;

        let choice = response.choices.into_iter().next()
            .ok_or_else(|| InferenceError::InvalidResponse("missing choice".into()))?;
        let text = choice.message.content
            .ok_or_else(|| InferenceError::InvalidResponse("choice has no text".into()))?;

        Ok(GenerationResult {
            text,
            finish_reason: Some(choice.finish_reason),
            usage: map_usage(response.usage),
            elapsed: started.elapsed(),
        })
    }

    async fn generate_stream(&self, request: GenerationRequest) -> Result<mpsc::Receiver<GenerationEvent>> {
        if request.messages.is_empty() {
            return Err(InferenceError::InvalidRequest("at least one message is required".into()));
        }

        let model = Arc::clone(&self.model);
        let (tx, rx) = mpsc::channel(32);

        // mistral.rs treats temperature below this threshold as "no
        // temperature" and routes to sample_argmax internally (its own
        // threshold), which on the 0.8 line never applies softmax before
        // taking the log — so we know with certainty, not just a heuristic,
        // when the broken path applies. Fixed in mistralrs 0.9.3, but we're
        // pinned back to 0.8 for now: 0.9.3's git-pinned candle-nn commit
        // has a confirmed ~190x CPU decode regression for non-quantized
        // (BF16) GGUF models via the accelerate feature (measured: 9.50
        // tok/s on 0.8.1's candle 0.10.2 vs 0.05 tok/s on 0.9.3's candle
        // 0.11.0-dev, same model/prompt) — a bigger cost than this bug for
        // this app's use case. Gated behind the `greedy-logprobs` Cargo
        // feature (off by default) so a future version bump only needs a
        // Cargo.toml change, not a source edit.
        #[cfg(feature = "greedy-logprobs")]
        let is_greedy = false;
        #[cfg(not(feature = "greedy-logprobs"))]
        let is_greedy = request.options.temperature < 1e-7;

        tokio::spawn(async move {
            let chat_request = build_chat_request(&request);
            let started = Instant::now();

            let mut stream = match model.stream_chat_request(chat_request).await {
                Ok(stream) => stream,
                Err(error) => {
                    let _ = tx.send(GenerationEvent::Error { message: error.to_string() }).await;
                    return;
                }
            };

            // mistral.rs' streaming path does not reliably emit a terminal
            // `Response::Done` — evidence from a real run: `Chunk`s arrive
            // and stream out fine, then `stream.next()` just returns `None`
            // with no Done ever seen. The final chunk instead carries
            // `usage`/`finish_reason` (per its own doc comment, "set on the
            // final chunk"). So: accumulate the text ourselves and treat
            // stream exhaustion as the real end-of-generation signal,
            // synthesizing `Done` from what we tracked. If a `Done` *does*
            // arrive on some path, honor it directly instead.
            let mut accumulated = String::new();
            let mut finish_reason = None;
            let mut usage = None;

            while let Some(response) = stream.next().await {
                match response {
                    Response::Chunk(chunk) => {
                        if chunk.usage.is_some() {
                            usage = chunk.usage;
                        }
                        if let Some(choice) = chunk.choices.into_iter().next() {
                            let logprob = if is_greedy { None } else { choice.logprobs.and_then(valid_logprob) };
                            if let Some(delta) = choice.delta.content {
                                if !delta.is_empty() {
                                    accumulated.push_str(&delta);
                                    if tx.send(GenerationEvent::Token { delta, logprob }).await.is_err() {
                                        return;
                                    }
                                }
                            }
                            if choice.finish_reason.is_some() {
                                finish_reason = choice.finish_reason;
                            }
                        }
                    }
                    Response::Done(final_response) => {
                        let choice = final_response.choices.into_iter().next();
                        let text = choice.as_ref().and_then(|c| c.message.content.clone()).unwrap_or(accumulated);
                        let result = GenerationResult {
                            text,
                            finish_reason: choice.map(|c| c.finish_reason),
                            usage: map_usage(final_response.usage),
                            elapsed: started.elapsed(),
                        };
                        let _ = tx.send(GenerationEvent::Done { result }).await;
                        return;
                    }
                    Response::ModelError(message, _) => {
                        let _ = tx.send(GenerationEvent::Error { message }).await;
                        return;
                    }
                    Response::InternalError(error) | Response::ValidationError(error) => {
                        let _ = tx.send(GenerationEvent::Error { message: error.to_string() }).await;
                        return;
                    }
                    // Completion/image/speech/raw responses don't apply to text chat.
                    _ => {}
                }
            }

            let result = GenerationResult {
                text: accumulated,
                finish_reason,
                usage: usage.map(map_usage).unwrap_or_default(),
                elapsed: started.elapsed(),
            };
            let _ = tx.send(GenerationEvent::Done { result }).await;
        });

        Ok(rx)
    }

    async fn tokenize(&self, text: &str) -> Result<Vec<u32>> {
        self.model
            .tokenize(Either::Right(text.to_owned()), None, false, false, None)
            .await
            .map_err(|error| InferenceError::Backend(error.to_string()))
    }
}
