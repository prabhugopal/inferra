<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { Download, Heart, Clock, FileStack, ExternalLink, Search } from "lucide-svelte";
  import {
    searchHuggingFaceModels,
    listHuggingFaceGgufFiles,
    type HfModelSummary,
    type HfGgufFile,
    type HfSort,
  } from "./api";
  import { loadPickedModel, isActiveModel } from "./modelLoad";
  import { modelState } from "./state.svelte";
  import { settings, updateSettings } from "./settings.svelte";
  import { formatBytes, formatCompactCount, formatRelativeTime } from "./format";

  const SORTS: { value: HfSort; label: string }[] = [
    { value: "downloads", label: "Most downloads" },
    { value: "trending", label: "Trending" },
    { value: "likes", label: "Most likes" },
    { value: "recent", label: "Recently updated" },
  ];

  const CACHE_TTL_OPTIONS = [
    { minutes: 0, label: "Off" },
    { minutes: 1, label: "1 minute" },
    { minutes: 10, label: "10 minutes" },
    { minutes: 60, label: "1 hour" },
    { minutes: 1440, label: "1 day" },
  ];

  function cacheTtlSeconds(): number {
    return settings.hubCacheTtlMinutes * 60;
  }

  let query = $state("");
  let sort = $state<HfSort>("downloads");
  let results = $state<HfModelSummary[]>([]);
  let loading = $state(false);
  let error = $state("");
  let searched = $state(false);

  let expanded = $state<string | null>(null);
  let files = $state<Record<string, HfGgufFile[]>>({});
  let filesLoading = $state<string | null>(null);
  let filesError = $state<Record<string, string>>({});
  let loadingFile = $state<string | null>(null);

  async function search() {
    loading = true;
    error = "";
    try {
      results = await searchHuggingFaceModels(query.trim(), sort, cacheTtlSeconds(), 24);
    } catch (err) {
      error = String(err);
      results = [];
    } finally {
      loading = false;
      searched = true;
    }
  }

  // Debounced so every keystroke doesn't fire a request against Hugging
  // Face's public API — search fires ~400ms after typing stops, sort
  // changes fire immediately since they're deliberate clicks, not typing.
  let debounceHandle: ReturnType<typeof setTimeout> | undefined;
  function onQueryInput() {
    clearTimeout(debounceHandle);
    debounceHandle = setTimeout(search, 400);
  }

  search();

  async function toggleExpand(repoId: string) {
    if (expanded === repoId) {
      expanded = null;
      return;
    }
    expanded = repoId;
    if (files[repoId]) return;
    filesLoading = repoId;
    try {
      files = { ...files, [repoId]: await listHuggingFaceGgufFiles(repoId, cacheTtlSeconds()) };
    } catch (err) {
      filesError = { ...filesError, [repoId]: String(err) };
    } finally {
      filesLoading = null;
    }
  }

  // Real convention across virtually every GGUF quantizer (bartowski,
  // unsloth, mradermacher, ...). Quantization's speed win comes from cutting
  // memory bandwidth (fewer bytes per weight), and that benefit scales with
  // model size — this session measured ~190x for a normal-sized model, but
  // for genuinely tiny ones (under ~200M params) the whole model already
  // fits in cache, so there's no bandwidth bottleneck to win back and F16's
  // plain dense matmul can actually be faster than Q4_K's dequant overhead.
  // So this is a real heuristic, not a guarantee — hence "usually" below.
  function isQuantized(filename: string): boolean {
    return !/\b(f16|f32|bf16)\b/i.test(filename);
  }

  // stopPropagation so this doesn't also toggle the card's expand/collapse
  // (its header row has its own onclick), and openUrl (not a plain <a href>)
  // so the link opens in the system's default browser instead of navigating
  // this app's own webview away.
  function viewOnHub(event: Event, repoId: string) {
    event.stopPropagation();
    openUrl(`https://huggingface.co/${repoId}`);
  }

  async function pick(repoId: string, file: HfGgufFile) {
    loadingFile = file.filename;
    try {
      await loadPickedModel({
        model_dir: repoId,
        gguf_file: file.filename,
        model_id: file.filename.replace(/\.gguf$/i, ""),
        tok_model_id: null,
        chat_template: null,
      });
    } finally {
      loadingFile = null;
    }
  }
</script>

<div class="browser">
  <div class="controls">
    <div class="search-input">
      <Search size={15} aria-hidden="true" />
      <input
        type="text"
        placeholder="Search Hugging Face Hub (e.g. llama, qwen, smollm)…"
        bind:value={query}
        oninput={onQueryInput}
      />
    </div>
    <select bind:value={sort} onchange={search}>
      {#each SORTS as s (s.value)}
        <option value={s.value}>{s.label}</option>
      {/each}
    </select>
  </div>
  <p class="hint">Showing GGUF, text-generation models only — the kind this app can actually load. Live from Hugging Face Hub, not a fixed list.</p>

  <details class="cache-settings">
    <summary>Cache settings</summary>
    <div class="fields">
      <label>
        Reuse results for
        <select
          value={settings.hubCacheTtlMinutes}
          onchange={(event) => updateSettings({ hubCacheTtlMinutes: Number((event.target as HTMLSelectElement).value) })}
        >
          {#each CACHE_TTL_OPTIONS as option (option.minutes)}
            <option value={option.minutes}>{option.label}</option>
          {/each}
        </select>
        before asking Hugging Face Hub again
      </label>
    </div>
  </details>

  {#if error}
    <p class="status-msg error">{error}</p>
  {:else if loading && results.length === 0}
    <div class="idle"><span>Searching Hugging Face Hub…</span></div>
  {:else if searched && results.length === 0}
    <div class="idle"><span>No GGUF models matched "{query}"</span></div>
  {:else}
    <div class="results">
      {#each results as model (model.id)}
        {@const isOpen = expanded === model.id}
        <div class="result" class:open={isOpen}>
          <!-- A plain div, not a <button>, is the flex container here:
               WebKit can fail to size a column-flex layout placed directly
               inside a <button> until something forces a repaint (hover,
               focus) — every row but the interacted one would render
               collapsed to near-zero height. -->
          <div
            class="result-header"
            role="button"
            tabindex="0"
            onclick={() => toggleExpand(model.id)}
            onkeydown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                toggleExpand(model.id);
              }
            }}
          >
            <div class="result-title">
              <span class="repo-id">{model.id}</span>
              {#if model.pipeline_tag}<span class="badge task">{model.pipeline_tag}</span>{/if}
              <button
                type="button"
                class="hub-link"
                title="View {model.id} on Hugging Face"
                onclick={(event) => viewOnHub(event, model.id)}
              >
                huggingface.co <ExternalLink size={11} aria-hidden="true" />
              </button>
            </div>
            <div class="tags">
              {#if model.parameter_count}<span class="tag tag-params">{formatCompactCount(model.parameter_count)} params</span>{/if}
              {#if model.architecture}<span class="tag tag-arch">{model.architecture}</span>{/if}
              {#if model.context_length}<span class="tag tag-ctx">{formatCompactCount(model.context_length)} ctx</span>{/if}
            </div>
            <div class="result-stats">
              <span title="Downloads"><Download size={12} aria-hidden="true" /> {formatCompactCount(model.downloads)}</span>
              <span title="Likes"><Heart size={12} aria-hidden="true" /> {formatCompactCount(model.likes)}</span>
              {#if model.last_modified}
                <span title="Last updated"><Clock size={12} aria-hidden="true" /> {formatRelativeTime(model.last_modified)}</span>
              {/if}
              <span title="GGUF files available">
                <FileStack size={12} aria-hidden="true" />
                {model.gguf_file_count} file{model.gguf_file_count === 1 ? "" : "s"}
              </span>
            </div>
          </div>

          {#if isOpen}
            <div class="files">
              {#if filesLoading === model.id}
                <p class="hint">Loading files…</p>
              {:else if filesError[model.id]}
                <p class="status-msg error">{filesError[model.id]}</p>
              {:else}
                {#each files[model.id] ?? [] as file (file.filename)}
                  {@const active = isActiveModel({ model_dir: model.id, gguf_file: file.filename })}
                  <div class="file-row" class:active>
                    <span class="filename" title={file.filename}>{file.filename}</span>
                    <div class="file-meta">
                      <span class="quant-hint" class:slow={!isQuantized(file.filename)}>
                        {isQuantized(file.filename) ? "quantized" : "unquantized — usually slower on CPU"}
                      </span>
                      <span class="filesize">{formatBytes(file.size_bytes)}</span>
                      <button
                        type="button"
                        onclick={() => pick(model.id, file)}
                        disabled={active || loadingFile === file.filename}
                      >
                        {active ? "Loaded" : loadingFile === file.filename ? "Loading…" : "Load"}
                      </button>
                    </div>
                  </div>
                {/each}
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .browser {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
  }
  .controls {
    display: flex;
    gap: 0.6rem;
  }
  .controls select {
    height: 2.5rem;
    box-sizing: border-box;
    font-size: 0.82rem;
    font-family: inherit;
    padding: 0 0.8em;
    border-radius: 8px;
    background: var(--surface-raised);
    color: var(--ink);
    border: 1px solid var(--baseline);
  }
  .search-input {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 0.5em;
    height: 2.5rem;
    box-sizing: border-box;
    padding: 0 0.8em;
    border-radius: 8px;
    background: var(--surface-raised);
    border: 1px solid var(--baseline);
    color: var(--ink-muted);
  }
  .search-input:focus-within {
    outline: 2px solid var(--signal);
    outline-offset: 1px;
  }
  .search-input input {
    flex: 1;
    height: 100%;
    box-sizing: border-box;
    border: none;
    background: transparent;
    color: var(--ink);
    font-size: 0.82rem;
    font-family: inherit;
    padding: 0;
  }
  .search-input input:focus-visible {
    outline: none;
  }
  .controls select:focus-visible {
    outline: 2px solid var(--signal);
    outline-offset: 1px;
  }
  .controls select {
    appearance: none;
    padding-right: 2.2em;
    background-image: linear-gradient(45deg, transparent 50%, var(--ink-muted) 50%),
      linear-gradient(135deg, var(--ink-muted) 50%, transparent 50%);
    background-position:
      calc(100% - 1.1em) center,
      calc(100% - 0.8em) center;
    background-size: 0.3em 0.3em;
    background-repeat: no-repeat;
  }
  .hint {
    margin: 0;
    font-size: 0.72rem;
    color: var(--ink-muted);
  }
  .cache-settings summary {
    font-size: 0.72rem;
    color: var(--ink-muted);
    cursor: pointer;
  }
  .cache-settings .fields {
    margin-top: 0.5rem;
  }
  .cache-settings label {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.75rem;
    color: var(--ink-muted);
    flex-wrap: wrap;
  }
  .cache-settings select {
    height: 2rem;
    font-size: 0.75rem;
    padding: 0 0.6em;
    border-radius: 6px;
    background: var(--surface-raised);
    color: var(--ink);
    border: 1px solid var(--baseline);
  }
  .status-msg.error {
    color: var(--critical);
    font-size: 0.85rem;
  }
  .idle {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 100px;
    padding: 0.6em 1.2em;
    border: 1px dashed var(--baseline);
    border-radius: 8px;
    color: var(--ink-muted);
    font-size: 0.78rem;
  }
  .results {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    /* Without this, grid's default `stretch` forces every card in a row to
       match the row's tallest neighbor — then `.result`'s `overflow: hidden`
       clips whatever content doesn't fit the shorter cards' natural height,
       which is why most cards were showing only a title and nothing else. */
    align-items: start;
    gap: 0.6rem;
    max-height: 620px;
    overflow-y: auto;
    align-content: start;
  }
  .result {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--baseline);
    border-radius: 10px;
    overflow: hidden;
  }
  .result.open {
    grid-column: span 2;
  }
  .result-header {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    align-items: flex-start;
    padding: 0.7em 0.9em;
    background: var(--surface-raised);
    text-align: left;
    cursor: pointer;
  }
  .result-header:focus-visible {
    outline: 2px solid var(--signal);
    outline-offset: -2px;
  }
  .result-title {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    /* nowrap + the repo-id's own ellipsis below is what keeps every card's
       header a single line tall — a wrapped 2-line title was the actual
       cause of the uneven row gaps once align-items: start stopped forcing
       cards to match their tallest row-mate. */
    flex-wrap: nowrap;
    width: 100%;
    min-width: 0;
  }
  .repo-id {
    font-family: var(--font-mono);
    font-size: 0.85rem;
    color: var(--ink);
    font-weight: 600;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hub-link {
    display: inline-flex;
    align-items: center;
    gap: 0.3em;
    flex-shrink: 0;
    margin-left: auto;
    padding: 0.15em 0.5em;
    border-radius: 6px;
    font-size: 0.68rem;
    font-family: var(--font-mono);
    color: var(--ink-muted);
    background: transparent;
    border: 1px solid transparent;
  }
  .hub-link:hover {
    color: var(--signal);
    border-color: var(--signal);
    filter: none;
  }
  .badge {
    font-size: 0.65rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    padding: 0.2em 0.55em;
    border-radius: 999px;
    background: var(--surface);
    color: var(--ink-muted);
    border: 1px solid var(--baseline);
  }
  .badge.task {
    color: var(--signal);
    border-color: var(--signal);
    background: var(--surface);
    background: color-mix(in srgb, var(--signal) 14%, var(--surface));
  }
  .tags {
    display: flex;
    gap: 0.4rem;
    flex-wrap: wrap;
  }
  .tag {
    font-size: 0.68rem;
    font-weight: 600;
    padding: 0.2em 0.5em;
    border-radius: 6px;
    font-family: var(--font-mono);
  }
  .tag-params {
    color: var(--good);
    background: var(--surface);
    background: color-mix(in srgb, var(--good) 16%, var(--surface));
  }
  .tag-arch {
    color: var(--accent-2);
    background: var(--surface);
    background: color-mix(in srgb, var(--accent-2) 16%, var(--surface));
  }
  .tag-ctx {
    color: var(--ink-muted);
    background: var(--surface);
  }
  .result-stats {
    display: flex;
    gap: 0.5rem 0.9rem;
    flex-wrap: wrap;
    font-size: 0.7rem;
    color: var(--ink-muted);
    font-family: var(--font-mono);
  }
  .result-stats span {
    display: inline-flex;
    align-items: center;
    gap: 0.3em;
  }
  .files {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: 0.7em 0.9em;
    border-top: 1px solid var(--baseline);
  }
  .file-row {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    padding: 0.4em 0;
    font-size: 0.78rem;
  }
  .file-row:not(:last-child) {
    border-bottom: 1px solid var(--baseline);
  }
  .file-row.active .filename {
    color: var(--signal);
  }
  .filename {
    font-family: var(--font-mono);
    color: var(--ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .file-meta {
    display: flex;
    align-items: center;
    gap: 0.7rem;
  }
  .quant-hint {
    font-size: 0.7rem;
    color: var(--good);
    white-space: nowrap;
  }
  .quant-hint.slow {
    color: var(--critical);
  }
  .filesize {
    font-family: var(--font-mono);
    color: var(--ink-muted);
    white-space: nowrap;
    margin-left: auto;
  }
  .file-meta button {
    font-size: 0.72rem;
    padding: 0.3em 0.7em;
  }
</style>
