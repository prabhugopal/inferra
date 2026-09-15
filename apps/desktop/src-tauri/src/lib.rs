//! Tauri commands are thin wrappers around `inferra_ui_bridge::FrontendBridge`.
//!
//! This crate owns no model configuration: model path, GGUF file, tokenizer
//! source, and chat template all come from the frontend via `load_model`, so
//! the desktop shell stays a UI over the same backend-independent contract
//! the CLI uses.

use inferra_app::InferenceService;
use inferra_core::ModelInfo;
use inferra_gguf::{EmbeddingPoint, GgufComposition};
use inferra_hub::{GgufFile, ModelSummary, SortBy};
use inferra_mistral::MistralRsBackend;
use inferra_ui_bridge::{FrontendBridge, GenerateCommand, GenerateStreamEvent, GenerateView};
use std::sync::{Arc, Mutex};
use tauri::ipc::Channel;

#[derive(Default)]
struct AppState {
    bridge: Mutex<Option<FrontendBridge>>,
}

#[derive(serde::Deserialize)]
struct ModelConfig {
    model_dir: String,
    gguf_file: String,
    model_id: String,
    tok_model_id: Option<String>,
    chat_template: Option<String>,
}

#[tauri::command]
async fn load_model(config: ModelConfig, state: tauri::State<'_, AppState>) -> Result<ModelInfo, String> {
    let backend = MistralRsBackend::load(
        config.model_dir,
        config.gguf_file,
        config.model_id,
        config.tok_model_id,
        config.chat_template,
    )
    .await
    .map_err(|error| error.to_string())?;

    let bridge = FrontendBridge::new(InferenceService::new(Arc::new(backend)));
    let info = bridge.model_info();

    *state.bridge.lock().map_err(|_| "app state lock poisoned".to_string())? = Some(bridge);
    Ok(info)
}

#[tauri::command]
fn model_info(state: tauri::State<'_, AppState>) -> Result<Option<ModelInfo>, String> {
    let guard = state.bridge.lock().map_err(|_| "app state lock poisoned".to_string())?;
    Ok(guard.as_ref().map(FrontendBridge::model_info))
}

#[tauri::command]
async fn generate(command: GenerateCommand, state: tauri::State<'_, AppState>) -> Result<GenerateView, String> {
    let bridge = {
        let guard = state.bridge.lock().map_err(|_| "app state lock poisoned".to_string())?;
        guard.clone().ok_or_else(|| "no model loaded; call load_model first".to_string())?
    };
    bridge.generate(command).await
}

#[tauri::command]
async fn generate_stream(
    command: GenerateCommand,
    on_event: Channel<GenerateStreamEvent>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let bridge = {
        let guard = state.bridge.lock().map_err(|_| "app state lock poisoned".to_string())?;
        guard.clone().ok_or_else(|| "no model loaded; call load_model first".to_string())?
    };
    let mut rx = bridge.generate_stream(command).await?;
    while let Some(event) = rx.recv().await {
        // A send failure here previously vanished silently, so a broken
        // event (e.g. one that failed to serialize) looked to the frontend
        // like generation just stopped with no result.
        if let Err(error) = on_event.send(event) {
            return Err(format!("failed to deliver stream event to frontend: {error}"));
        }
    }
    Ok(())
}

/// Default embedding tensor name across the llama-family GGUF exports this
/// app has been tested against; overridable since GGUF doesn't standardize
/// a name across every architecture.
const DEFAULT_EMBEDDING_TENSOR: &str = "token_embd.weight";
const DEFAULT_EMBEDDING_SAMPLE_SIZE: usize = 500;

/// GGUF composition/embedding work is synchronous CPU work — sometimes
/// heavy (dequantizing an embedding matrix, a brute-force similarity search
/// over the whole vocabulary). Running it directly on a `#[tauri::command]`
/// blocks the async runtime thread that also services the webview's IPC and
/// window events, which is what made the whole window (not just the plot)
/// look frozen — a spinning-cursor "not responding" state, not a crash.
/// `spawn_blocking` moves it to a dedicated thread so the rest of the UI
/// stays interactive while it runs.
async fn run_blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|error| format!("background task panicked: {error}"))?
}

#[tauri::command]
async fn inspect_gguf_composition(model_dir: String, gguf_file: String) -> Result<GgufComposition, String> {
    run_blocking(move || {
        let path = inferra_hub::resolve_gguf_path(&model_dir, &gguf_file).map_err(|error| error.to_string())?;
        inferra_gguf::read_composition(&path).map_err(|error| error.to_string())
    })
    .await
}

#[tauri::command]
async fn sample_gguf_embeddings(
    model_dir: String,
    gguf_file: String,
    tensor_name: Option<String>,
    sample_size: Option<usize>,
) -> Result<Vec<EmbeddingPoint>, String> {
    run_blocking(move || {
        let path = inferra_hub::resolve_gguf_path(&model_dir, &gguf_file).map_err(|error| error.to_string())?;
        let tensor_name = tensor_name.unwrap_or_else(|| DEFAULT_EMBEDDING_TENSOR.to_string());
        let sample_size = sample_size.unwrap_or(DEFAULT_EMBEDDING_SAMPLE_SIZE);
        inferra_gguf::sample_embeddings(&path, &tensor_name, sample_size).map_err(|error| error.to_string())
    })
    .await
}

#[tauri::command]
async fn sample_gguf_embeddings_near(
    model_dir: String,
    gguf_file: String,
    seed_token_ids: Vec<u32>,
    tensor_name: Option<String>,
    sample_size: Option<usize>,
) -> Result<Vec<EmbeddingPoint>, String> {
    run_blocking(move || {
        let path = inferra_hub::resolve_gguf_path(&model_dir, &gguf_file).map_err(|error| error.to_string())?;
        let tensor_name = tensor_name.unwrap_or_else(|| DEFAULT_EMBEDDING_TENSOR.to_string());
        let sample_size = sample_size.unwrap_or(DEFAULT_EMBEDDING_SAMPLE_SIZE);
        inferra_gguf::sample_embeddings_near(&path, &tensor_name, &seed_token_ids, sample_size)
            .map_err(|error| error.to_string())
    })
    .await
}

#[tauri::command]
async fn tokenize_text(text: String, state: tauri::State<'_, AppState>) -> Result<Vec<u32>, String> {
    let bridge = {
        let guard = state.bridge.lock().map_err(|_| "app state lock poisoned".to_string())?;
        guard.clone().ok_or_else(|| "no model loaded; call load_model first".to_string())?
    };
    bridge.tokenize(&text).await
}

#[tauri::command]
async fn search_huggingface_models(
    query: String,
    sort: String,
    limit: Option<usize>,
    cache_ttl_seconds: u64,
) -> Result<Vec<ModelSummary>, String> {
    let sort = match sort.as_str() {
        "downloads" => SortBy::Downloads,
        "likes" => SortBy::Likes,
        "trending" => SortBy::Trending,
        "recent" => SortBy::RecentlyUpdated,
        other => return Err(format!("unknown sort option '{other}'")),
    };
    // `{error:#}` (anyhow's alternate Display) walks the whole `.context()`
    // chain — plain `{error}`/`.to_string()` prints only the outermost line,
    // silently dropping the actual cause (status code, timeout, etc.).
    inferra_hub::search_models(&query, sort, limit.unwrap_or(24), std::time::Duration::from_secs(cache_ttl_seconds))
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn list_huggingface_gguf_files(repo_id: String, cache_ttl_seconds: u64) -> Result<Vec<GgufFile>, String> {
    inferra_hub::list_gguf_files(&repo_id, std::time::Duration::from_secs(cache_ttl_seconds))
        .await
        .map_err(|error| format!("{error:#}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            load_model,
            model_info,
            generate,
            generate_stream,
            inspect_gguf_composition,
            sample_gguf_embeddings,
            sample_gguf_embeddings_near,
            tokenize_text,
            search_huggingface_models,
            list_huggingface_gguf_files
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
