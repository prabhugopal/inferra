// Shared reactive model status so the header, sidebar, chat panel, and
// model inspector all agree on what's loaded, without prop-drilling.
import type { ModelInfo, KvCacheEstimate } from "./api";

export type ModelStatus = "unloaded" | "loading" | "ready" | "error";

export const modelState = $state<{
  status: ModelStatus;
  info: ModelInfo | null;
  error: string | null;
  // The GGUF file path behind `info` — the inspector reads straight from
  // this file, not from anything mistral.rs already loaded into memory.
  modelDir: string | null;
  ggufFile: string | null;
  // Fetched once per load (cheap — GGUF metadata only, not tensor data) so
  // the live per-token KV cache readout works without requiring a visit to
  // the Model Inspector tab first.
  kvCache: KvCacheEstimate | null;
}>({
  status: "unloaded",
  info: null,
  error: null,
  modelDir: null,
  ggufFile: null,
  kvCache: null,
});
