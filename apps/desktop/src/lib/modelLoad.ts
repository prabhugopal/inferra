// Shared "load a model and update the app's shared model state" logic —
// used by both the manual/local-file loader (ModelLoader.svelte) and the
// Hugging Face browser (ModelBrowser.svelte), so there's exactly one place
// that knows how to go from a ModelConfig to a loaded, remembered model.
import { loadModel, inspectGgufComposition, type ModelConfig } from "./api";
import { modelState } from "./state.svelte";
import { rememberModel, recentModels } from "./persist";

export function isActiveModel(config: { model_dir: string; gguf_file: string }): boolean {
  return (
    modelState.status === "ready" &&
    modelState.modelDir === config.model_dir &&
    modelState.ggufFile === config.gguf_file
  );
}

export async function loadPickedModel(config: ModelConfig): Promise<void> {
  // Picking the model that's already active (e.g. clicking its Recent chip
  // again) is a no-op instead of paying for a full reload.
  if (isActiveModel(config)) return;
  modelState.status = "loading";
  modelState.error = null;
  modelState.kvCache = null;
  try {
    modelState.info = await loadModel(config);
    modelState.status = "ready";
    modelState.modelDir = config.model_dir;
    modelState.ggufFile = config.gguf_file;
    rememberModel(config);
    // Best-effort: the live KV-cache readout is a nice-to-have, not core to
    // having a usable model, so a failure here (nothing else reads this GGUF
    // composition on load today) shouldn't fail the whole model load.
    try {
      const composition = await inspectGgufComposition(config.model_dir, config.gguf_file);
      modelState.kvCache = composition.kv_cache;
    } catch {
      modelState.kvCache = null;
    }
  } catch (err) {
    modelState.status = "error";
    modelState.error = String(err);
  }
}

// The label is purely a display string — mistral.rs never sees it again
// after load — so renaming the active model updates local state directly
// instead of going through loadPickedModel, which would otherwise no-op
// (isActiveModel only compares model_dir/gguf_file) or, if it didn't no-op,
// pay for a full pointless reload just to change a string.
export function renameActiveModel(newLabel: string): void {
  const label = newLabel.trim();
  if (!label || modelState.status !== "ready" || !modelState.info) return;
  modelState.info = { ...modelState.info, model_id: label };
  if (modelState.modelDir && modelState.ggufFile) {
    const existing = recentModels().find(
      (profile) => profile.model_dir === modelState.modelDir && profile.gguf_file === modelState.ggufFile,
    );
    rememberModel({
      model_dir: modelState.modelDir,
      gguf_file: modelState.ggufFile,
      model_id: label,
      tok_model_id: existing?.tok_model_id ?? null,
      chat_template: existing?.chat_template ?? null,
    });
  }
}
