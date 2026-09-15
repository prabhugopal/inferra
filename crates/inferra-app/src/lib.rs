use inferra_core::{
    GenerationEvent, GenerationRequest, GenerationResult, InferenceBackend, ModelInfo, Result,
};
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct InferenceService {
    backend: Arc<dyn InferenceBackend>,
}

impl InferenceService {
    pub fn new(backend: Arc<dyn InferenceBackend>) -> Self { Self { backend } }
    pub fn model_info(&self) -> ModelInfo { self.backend.model_info() }
    pub async fn generate(&self, request: GenerationRequest) -> Result<GenerationResult> {
        self.backend.generate(request).await
    }
    pub async fn generate_stream(&self, request: GenerationRequest) -> Result<mpsc::Receiver<GenerationEvent>> {
        self.backend.generate_stream(request).await
    }
    pub async fn tokenize(&self, text: &str) -> Result<Vec<u32>> {
        self.backend.tokenize(text).await
    }
}
