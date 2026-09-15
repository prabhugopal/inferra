// Typed wrapper around the Tauri commands exposed by src-tauri/src/lib.rs.
// Mirrors the DTOs in crates/inferra-core and crates/inferra-ui-bridge — this
// is the one place that needs to know the Rust command names and shapes.
import { invoke, Channel } from "@tauri-apps/api/core";

export interface ModelInfo {
  backend: string;
  model_id: string;
  endpoint: string | null;
}

export interface ModelConfig {
  model_dir: string;
  gguf_file: string;
  model_id: string;
  tok_model_id: string | null;
  chat_template: string | null;
}

export interface GenerationOptions {
  max_new_tokens: number;
  temperature: number;
  top_p: number;
  seed: number | null;
}

export interface GenerateView {
  text: string;
  elapsed_ms: number;
  prompt_tokens: number | null;
  completion_tokens: number | null;
  // mistral.rs separates prefill from decode internally and reports both —
  // these mirror that breakdown rather than one blended end-to-end rate.
  prompt_tokens_per_sec: number | null;
  completion_tokens_per_sec: number | null;
  prompt_time_sec: number | null;
  completion_time_sec: number | null;
}

export function loadModel(config: ModelConfig): Promise<ModelInfo> {
  return invoke("load_model", { config });
}

export function fetchModelInfo(): Promise<ModelInfo | null> {
  return invoke("model_info");
}

export function generate(prompt: string, options: Partial<GenerationOptions>): Promise<GenerateView> {
  return invoke("generate", { command: { prompt, options } });
}

export interface TokenLogprob {
  token: string;
  logprob: number;
}

// Mirrors inferra_ui_bridge::GenerateStreamEvent's `#[serde(tag = "type")]` wire shape.
export type GenerateStreamEvent =
  | { type: "Token"; delta: string; logprob: TokenLogprob | null }
  | { type: "Done"; view: GenerateView }
  | { type: "Error"; message: string };

export function generateStream(
  prompt: string,
  options: Partial<GenerationOptions>,
  onEvent: (event: GenerateStreamEvent) => void,
): Promise<void> {
  const channel = new Channel<GenerateStreamEvent>();
  channel.onmessage = onEvent;
  return invoke("generate_stream", { command: { prompt, options }, onEvent: channel });
}

// Mirrors inferra_gguf's GgufComposition/EmbeddingPoint — real bytes read
// from the file (candle-core's own GGUF reader), not derived/estimated.
export interface GgufMetadataEntry {
  key: string;
  value: string;
}

export interface GgufTensorSummary {
  name: string;
  shape: number[];
  dtype: string;
  byte_size: number;
}

export interface SpecialToken {
  label: string;
  text: string;
}

export interface GgufComposition {
  version: number;
  file_size: number;
  header_size: number;
  metadata_and_descriptors_size: number;
  tensor_data_offset: number;
  tensor_data_size: number;
  metadata: GgufMetadataEntry[];
  tensors: GgufTensorSummary[];
  chat_template: string | null;
  special_tokens: SpecialToken[];
  kv_cache: KvCacheEstimate | null;
}

export interface KvCacheEstimate {
  bytes_per_token: number;
  context_length: number | null;
  bytes_at_max_context: number | null;
}

export interface EmbeddingPoint {
  token: string;
  x: number;
  y: number;
  z: number;
  // Whether this is one of the seed tokens from real text (vs. a neighbor
  // found around it, or an evenly-spaced sample when no text was given).
  is_seed: boolean;
}

export function inspectGgufComposition(modelDir: string, ggufFile: string): Promise<GgufComposition> {
  return invoke("inspect_gguf_composition", { modelDir, ggufFile });
}

export function sampleGgufEmbeddings(
  modelDir: string,
  ggufFile: string,
  tensorName?: string,
  sampleSize?: number,
): Promise<EmbeddingPoint[]> {
  return invoke("sample_gguf_embeddings", { modelDir, ggufFile, tensorName, sampleSize });
}

export function sampleGgufEmbeddingsNear(
  modelDir: string,
  ggufFile: string,
  seedTokenIds: number[],
  tensorName?: string,
  sampleSize?: number,
): Promise<EmbeddingPoint[]> {
  return invoke("sample_gguf_embeddings_near", { modelDir, ggufFile, seedTokenIds, tensorName, sampleSize });
}

// Raw token IDs for `text`, using the loaded model's own tokenizer — no
// chat template, no special tokens. Requires a model to already be loaded.
export function tokenizeText(text: string): Promise<number[]> {
  return invoke("tokenize_text", { text });
}

// Mirrors inferra_hub::ModelSummary/GgufFile — every field here is real
// data from Hugging Face Hub's own API, not a hardcoded catalog.
export interface HfModelSummary {
  id: string;
  pipeline_tag: string | null;
  downloads: number;
  likes: number;
  last_modified: string | null;
  gguf_file_count: number;
  parameter_count: number | null;
  architecture: string | null;
  context_length: number | null;
}

export interface HfGgufFile {
  filename: string;
  size_bytes: number;
}

export type HfSort = "downloads" | "likes" | "trending" | "recent";

export function searchHuggingFaceModels(
  query: string,
  sort: HfSort,
  cacheTtlSeconds: number,
  limit?: number,
): Promise<HfModelSummary[]> {
  return invoke("search_huggingface_models", { query, sort, limit, cacheTtlSeconds });
}

export function listHuggingFaceGgufFiles(repoId: string, cacheTtlSeconds: number): Promise<HfGgufFile[]> {
  return invoke("list_huggingface_gguf_files", { repoId, cacheTtlSeconds });
}
