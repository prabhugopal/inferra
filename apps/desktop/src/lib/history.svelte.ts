// Session-only generation history, shared by Chat.svelte (writer) and the
// visualization components (readers). Not persisted — it's an observability
// aid for the current run, not data worth keeping across restarts.
import type { GenerateView } from "./api";
import { modelState } from "./state.svelte";

export interface GenerationRecord {
  id: number;
  // The active model's label at the moment this generation ran — so a
  // model switch mid-session is visible in the log/charts instead of
  // silently blending into the same trend line as a different model's runs.
  modelId: string | null;
  prompt: string;
  responseText: string;
  elapsedMs: number;
  promptTokens: number | null;
  completionTokens: number | null;
  // Straight from mistral.rs's own prefill/decode split — not recomputed here.
  promptTokensPerSec: number | null;
  completionTokensPerSec: number | null;
  promptTimeSec: number | null;
  completionTimeSec: number | null;
}

// Retained regardless of the user's configured display size (settings.ts),
// so raising that size later can recover recent-but-hidden generations
// instead of only affecting runs recorded after the change.
const HARD_CAP = 50;

export const history = $state<{ records: GenerationRecord[] }>({ records: [] });

let nextId = 1;

export function recordGeneration(prompt: string, view: GenerateView): void {
  const record: GenerationRecord = {
    id: nextId++,
    modelId: modelState.info?.model_id ?? null,
    prompt,
    responseText: view.text,
    elapsedMs: view.elapsed_ms,
    promptTokens: view.prompt_tokens,
    completionTokens: view.completion_tokens,
    promptTokensPerSec: view.prompt_tokens_per_sec,
    completionTokensPerSec: view.completion_tokens_per_sec,
    promptTimeSec: view.prompt_time_sec,
    completionTimeSec: view.completion_time_sec,
  };
  history.records = [...history.records, record].slice(-HARD_CAP);
}
