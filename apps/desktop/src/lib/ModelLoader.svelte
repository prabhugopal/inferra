<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { dirname, basename } from "@tauri-apps/api/path";
  import type { ModelConfig } from "./api";
  import { modelState } from "./state.svelte";
  import { recentModels, forgetModel, type ModelProfile } from "./persist";
  import { isActiveModel, loadPickedModel, renameActiveModel } from "./modelLoad";
  import { FolderOpen, X, Check, PlayCircle, Loader2 } from "lucide-svelte";

  let modelDir = $state("");
  let ggufFile = $state("");
  let modelId = $state("local-gguf");
  let tokModelId = $state("");
  let chatTemplate = $state("");
  const initialRecent = recentModels();
  let recent = $state<ModelProfile[]>(initialRecent);

  const mostRecent = initialRecent.at(0);
  if (mostRecent) {
    modelDir = mostRecent.model_dir;
    ggufFile = mostRecent.gguf_file;
    modelId = mostRecent.model_id;
    tokModelId = mostRecent.tok_model_id ?? "";
    chatTemplate = mostRecent.chat_template ?? "";
  }

  async function loadWithConfig(config: ModelConfig) {
    await loadPickedModel(config);
    recent = recentModels();
  }

  // Auto-load the last-used model on startup — the app has no persisted
  // in-memory state across launches, so "Recent" would otherwise require a
  // click every single time even for the model you just used.
  if (mostRecent) {
    loadWithConfig(mostRecent);
  }

  function currentConfig(): ModelConfig {
    return {
      model_dir: modelDir,
      gguf_file: ggufFile,
      model_id: modelId,
      tok_model_id: tokModelId.trim() ? tokModelId.trim() : null,
      chat_template: chatTemplate.trim() ? chatTemplate.trim() : null,
    };
  }

  // Recent profiles and a freshly-picked GGUF file both load immediately —
  // filling the fields without loading was the previous, easy-to-miss step
  // that left the prompt box disabled with no obvious next action.
  function applyProfile(profile: ModelProfile) {
    modelDir = profile.model_dir;
    ggufFile = profile.gguf_file;
    modelId = profile.model_id;
    tokModelId = profile.tok_model_id ?? "";
    chatTemplate = profile.chat_template ?? "";
    loadWithConfig(profile);
  }

  function removeProfile(profile: ModelProfile, event: Event) {
    event.stopPropagation();
    forgetModel(profile);
    recent = recentModels();
  }

  async function browseGgufFile() {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "GGUF model", extensions: ["gguf"] }],
    });
    if (typeof selected !== "string") return;
    modelDir = await dirname(selected);
    ggufFile = await basename(selected);
    // Picking a file always renames the label to match it — a label left
    // over from whatever was loaded before (including a stale "local-gguf"
    // from an early session) would otherwise silently stick around.
    modelId = ggufFile.replace(/\.gguf$/i, "");
    await loadWithConfig(currentConfig());
  }

  async function browseChatTemplate() {
    const selected = await open({ multiple: false, directory: false });
    if (typeof selected === "string") chatTemplate = selected;
  }

  function submit(event: Event) {
    event.preventDefault();
    loadWithConfig(currentConfig());
  }

  // True only while the label field has an edit that hasn't been saved or
  // discarded yet — this is what shows the tick/cross, and is unrelated to
  // the full form's own submit (which reloads the model; renaming does not).
  let labelDirty = $derived(
    modelState.status === "ready" &&
      modelId.trim() !== "" &&
      modelId.trim() !== modelState.info?.model_id,
  );

  function saveLabel() {
    renameActiveModel(modelId);
    recent = recentModels();
  }

  function discardLabel() {
    modelId = modelState.info?.model_id ?? modelId;
  }
</script>

<aside class="rail">
  <button type="button" class="pick" onclick={browseGgufFile}>
    <FolderOpen size={16} aria-hidden="true" />
    Choose a GGUF file…
  </button>

  {#if recent.length > 0}
    <div class="recent">
      <span class="section-label">Recent</span>
      <ul>
        {#each recent as profile (profile.model_dir + profile.gguf_file)}
          <li>
            <button
              type="button"
              class="chip"
              class:active={isActiveModel(profile)}
              onclick={() => applyProfile(profile)}
            >
              <span class="chip-name" title={profile.model_id}>{profile.model_id}</span>
              <span
                class="remove"
                role="button"
                tabindex="0"
                aria-label="Remove {profile.model_id} from recent models"
                onclick={(event) => removeProfile(profile, event)}
                onkeydown={(event) => {
                  if (event.key === "Enter" || event.key === " ") removeProfile(profile, event);
                }}
              >
                <X size={12} aria-hidden="true" />
              </span>
            </button>
          </li>
        {/each}
      </ul>
    </div>
  {/if}

  {#if modelState.status === "error" && modelState.error}
    <p class="status-msg error">{modelState.error}</p>
  {/if}

  <details class="advanced">
    <summary>Manual configuration</summary>
    <form onsubmit={submit}>
      <label>
        Model directory
        <input bind:value={modelDir} placeholder="/path/to/gguf/dir" required />
      </label>
      <label>
        GGUF file name
        <input bind:value={ggufFile} placeholder="model.Q4_K_M.gguf" required />
      </label>
      <label>
        Model label
        <div class="with-actions">
          <input bind:value={modelId} />
          {#if labelDirty}
            <button type="button" class="icon-btn confirm" title="Save label" onclick={saveLabel}>
              <Check size={14} aria-hidden="true" />
            </button>
            <button type="button" class="icon-btn cancel" title="Discard" onclick={discardLabel}>
              <X size={14} aria-hidden="true" />
            </button>
          {/if}
        </div>
      </label>
      <label>
        Tokenizer source <span class="hint">(HF repo id, if the GGUF has no embedded tokenizer)</span>
        <input bind:value={tokModelId} placeholder="org/model-name" />
      </label>
      <label>
        Chat template <span class="hint">(inline Jinja or a path)</span>
        <div class="with-browse">
          <input bind:value={chatTemplate} />
          <button type="button" onclick={browseChatTemplate}>
            <FolderOpen size={14} aria-hidden="true" />
            Browse…
          </button>
        </div>
      </label>
      <button type="submit" class="submit" disabled={modelState.status === "loading"}>
        {#if modelState.status === "loading"}
          <Loader2 size={15} class="spin" aria-hidden="true" />
          Loading…
        {:else}
          <PlayCircle size={15} aria-hidden="true" />
          Load
        {/if}
      </button>
    </form>
  </details>
</aside>

<style>
  .rail {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  .pick {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    justify-content: center;
    width: 100%;
    background: var(--signal);
    color: #ffffff;
    border: none;
    border-radius: 8px;
    padding: 0.65em 1em;
    font-size: 0.9rem;
    font-weight: 600;
    cursor: pointer;
  }
  .pick:hover {
    filter: brightness(1.08);
  }
  .section-label {
    display: block;
    font-size: 0.7rem;
    color: var(--ink-muted);
    margin-bottom: 0.4rem;
  }
  .recent ul {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .chip {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    gap: 0.4rem;
    background: var(--surface-raised);
    color: var(--ink);
    border: 1px solid transparent;
    border-radius: 6px;
    padding: 0.45em 0.6em;
    font-size: 0.82rem;
    text-align: left;
    cursor: pointer;
    font-family: var(--font-mono);
  }
  .chip:hover {
    border-color: var(--signal);
  }
  .chip.active {
    border-color: var(--signal);
    box-shadow: 0 0 0 1px var(--signal) inset;
  }
  .chip-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .remove {
    display: inline-flex;
    align-items: center;
    opacity: 0.5;
    padding: 0 0.15rem;
    flex-shrink: 0;
  }
  .remove:hover {
    opacity: 1;
  }
  .status-msg {
    font-size: 0.8rem;
    padding: 0.5em 0.6em;
    border-radius: 6px;
    line-height: 1.4;
  }
  .status-msg.error {
    background: color-mix(in srgb, var(--critical) 15%, transparent);
    color: var(--critical);
  }
  .advanced summary {
    font-size: 0.78rem;
    color: var(--ink-muted);
    cursor: pointer;
  }
  .advanced form {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    margin-top: 0.6rem;
  }
  .advanced label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.78rem;
    color: var(--ink-muted);
  }
  .advanced input {
    font-family: var(--font-mono);
    font-size: 0.78rem;
  }
  .hint {
    font-weight: 400;
    opacity: 0.8;
  }
  .with-browse {
    display: flex;
    gap: 0.4rem;
  }
  .with-browse input {
    flex: 1;
  }
  .with-browse button {
    display: inline-flex;
    align-items: center;
    gap: 0.35em;
    padding: 0.4em 0.7em;
    font-size: 0.78rem;
  }
  .with-actions {
    display: flex;
    gap: 0.35rem;
  }
  .with-actions input {
    flex: 1;
    min-width: 0;
  }
  .icon-btn {
    display: inline-flex;
    align-items: center;
    flex-shrink: 0;
    padding: 0 0.6em;
    font-size: 0.85rem;
    line-height: 1;
  }
  .icon-btn.confirm {
    color: var(--good);
    border-color: var(--good);
  }
  .icon-btn.cancel {
    color: var(--critical);
    border-color: var(--critical);
  }
  .submit {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.45em;
  }
  :global(.spin) {
    animation: spin 0.9s linear infinite;
  }
  @media (prefers-reduced-motion: reduce) {
    :global(.spin) {
      animation: none;
    }
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
