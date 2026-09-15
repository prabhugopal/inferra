<script lang="ts">
  import { modelState } from "./state.svelte";
  import { inspectGgufComposition, type GgufComposition } from "./api";
  import { formatBytes } from "./format";

  let composition = $state<GgufComposition | null>(null);
  let loading = $state(false);
  let error = $state("");

  // Without this, switching models leaves the previous model's composition
  // breakdown on screen with nothing marking it stale. Any change in which
  // model is active clears it, so a fresh "Inspect" is required per model.
  let lastModelKey = modelKey();
  $effect(() => {
    const key = modelKey();
    if (key !== lastModelKey) {
      lastModelKey = key;
      composition = null;
      error = "";
    }
  });

  function modelKey(): string | null {
    return modelState.modelDir && modelState.ggufFile ? `${modelState.modelDir}::${modelState.ggufFile}` : null;
  }

  async function load() {
    const dir = modelState.modelDir;
    const file = modelState.ggufFile;
    if (!dir || !file) return;
    loading = true;
    error = "";
    try {
      composition = await inspectGgufComposition(dir, file);
    } catch (err) {
      error = String(err);
      composition = null;
    } finally {
      loading = false;
    }
  }

  let anatomy = $derived(
    composition && [
      { label: "Header", bytes: composition.header_size, class: "header" },
      { label: "Metadata + tensor descriptors", bytes: composition.metadata_and_descriptors_size, class: "meta" },
      { label: "Tensor data", bytes: composition.tensor_data_size, class: "data" },
    ],
  );
  let maxTensorBytes = $derived(Math.max(1, ...(composition?.tensors.map((t) => t.byte_size) ?? [1])));

  // The number that actually makes the cost concrete: how the KV cache at
  // full context compares to the weights themselves — for a long-context
  // model this can rival or exceed the weights, which a stat line alone
  // doesn't make visually obvious.
  let weightsVsKv = $derived.by(() => {
    if (!composition?.kv_cache?.bytes_at_max_context) return null;
    const weights = composition.tensor_data_size;
    const kv = composition.kv_cache.bytes_at_max_context;
    const max = Math.max(weights, kv, 1);
    return [
      { label: "Model weights", bytes: weights, pct: (weights / max) * 100, colorClass: "signal" },
      { label: "KV cache at max context", bytes: kv, pct: (kv / max) * 100, colorClass: "critical" },
    ];
  });

  const QUANT_COLORS = ["signal", "accent-2", "good", "critical"];
  let quantization = $derived.by(() => {
    if (!composition) return null;
    const totals = new Map<string, number>();
    for (const t of composition.tensors) totals.set(t.dtype, (totals.get(t.dtype) ?? 0) + t.byte_size);
    return [...totals.entries()]
      .sort((a, b) => b[1] - a[1])
      .map(([dtype, bytes], i) => ({ dtype, bytes, colorClass: QUANT_COLORS[i % QUANT_COLORS.length] }));
  });

  function metaValue(c: GgufComposition, key: string): string | null {
    return c.metadata.find((e) => e.key === key)?.value ?? null;
  }

  // A model's declared "purpose" isn't a GGUF field — but real metadata
  // gets you most of the way there. Whether it ships a chat template is a
  // reliable, non-guessed signal: base/completion models don't have one,
  // instruction/chat-tuned models do.
  let about = $derived(
    composition && {
      name: metaValue(composition, "general.name") ?? metaValue(composition, "general.basename"),
      architecture: metaValue(composition, "general.architecture"),
      size: metaValue(composition, "general.size_label"),
      finetune: metaValue(composition, "general.finetune"),
      license: metaValue(composition, "general.license"),
      organization: metaValue(composition, "general.organization"),
      languages: metaValue(composition, "general.languages"),
      isChatTuned: composition.metadata.some((e) => e.key === "tokenizer.chat_template"),
    },
  );
</script>

<div class="inspector">
  <div class="controls">
    <button type="button" onclick={load} disabled={!modelState.modelDir || loading}>
      {loading ? "Inspecting…" : composition ? "Re-inspect" : "Inspect GGUF file"}
    </button>
  </div>

  {#if !modelState.modelDir}
    <div class="idle"><span>Load a model to inspect its GGUF file</span></div>
  {:else if error}
    <p class="status-msg error">{error}</p>
  {:else if composition && about}
    <section>
      <h3>About this model</h3>
      <div class="about">
        <div class="about-title">
          {about.name ?? modelState.ggufFile}
          <span class="badge" class:chat={about.isChatTuned}>
            {about.isChatTuned ? "Chat / instruction-tuned" : "Base / completion model"}
          </span>
        </div>
        <div class="about-facts">
          {#if about.architecture}<span><strong>Architecture</strong> {about.architecture}</span>{/if}
          {#if about.size}<span><strong>Size</strong> {about.size}</span>{/if}
          {#if about.organization}<span><strong>From</strong> {about.organization}</span>{/if}
          {#if about.license}<span><strong>License</strong> {about.license}</span>{/if}
          {#if about.languages}<span><strong>Languages</strong> {about.languages}</span>{/if}
          {#if about.finetune}<span><strong>Finetune</strong> {about.finetune}</span>{/if}
        </div>
        <p class="about-note">
          {about.isChatTuned
            ? "Ships a chat template (tokenizer.chat_template) — designed for multi-turn conversation, not just raw text completion."
            : "No chat template found — this is likely a base model, tuned for text completion rather than following instructions."}
        </p>
        {#if composition.special_tokens.length > 0}
          <div class="special-tokens">
            {#each composition.special_tokens as token (token.label)}
              <span class="token-chip"><strong>{token.label}</strong> <code>{token.text}</code></span>
            {/each}
          </div>
        {/if}
        {#if composition.chat_template}
          <details class="chat-template">
            <summary>Chat template ({composition.chat_template.length.toLocaleString()} chars)</summary>
            <pre>{composition.chat_template}</pre>
          </details>
        {/if}
      </div>
    </section>

    {#if composition.kv_cache}
      <section>
        <h3>KV cache <span class="hint">estimated at F16 — the actual runtime may choose differently</span></h3>
        <div class="about-facts">
          <span><strong>Per token</strong> {formatBytes(composition.kv_cache.bytes_per_token)}</span>
          {#if composition.kv_cache.context_length}
            <span><strong>Max context</strong> {composition.kv_cache.context_length.toLocaleString()} tokens</span>
          {/if}
          {#if composition.kv_cache.bytes_at_max_context}
            <span><strong>At max context</strong> {formatBytes(composition.kv_cache.bytes_at_max_context)}</span>
          {/if}
        </div>
        <p class="about-note">
          Every generated token adds a fixed-size slice to the KV cache — this is why memory (not just compute) grows with
          conversation length, and why long-context models are expensive to serve at scale.
        </p>
        {#if weightsVsKv}
          <div class="compare-bars">
            {#each weightsVsKv as row (row.label)}
              <div class="compare-row">
                <span class="compare-label">{row.label}</span>
                <div class="compare-track">
                  <div class="compare-fill quant-{row.colorClass}" style="width: {Math.max(row.pct, 1)}%"></div>
                </div>
                <span class="compare-value">{formatBytes(row.bytes)}</span>
              </div>
            {/each}
          </div>
        {/if}
      </section>
    {/if}

    <section>
      <h3>File anatomy <span class="hint">GGUF v{composition.version} · {formatBytes(composition.file_size)}</span></h3>
      <div class="anatomy-bar">
        {#each anatomy ?? [] as segment (segment.label)}
          {@const pct = (segment.bytes / composition.file_size) * 100}
          <div class="segment {segment.class}" style="width: {Math.max(pct, 0.3)}%" title="{segment.label}: {formatBytes(segment.bytes)}"></div>
        {/each}
      </div>
      <div class="anatomy-legend">
        {#each anatomy ?? [] as segment (segment.label)}
          <span class="key"><span class="swatch {segment.class}"></span>{segment.label} — {formatBytes(segment.bytes)}</span>
        {/each}
      </div>
    </section>

    {#if quantization && quantization.length > 0}
      <section>
        <h3>Quantization <span class="hint">tensor bytes by dtype</span></h3>
        <div class="anatomy-bar">
          {#each quantization as q (q.dtype)}
            {@const pct = (q.bytes / composition.tensor_data_size) * 100}
            <div class="segment quant-{q.colorClass}" style="width: {Math.max(pct, 0.3)}%" title="{q.dtype}: {formatBytes(q.bytes)}"></div>
          {/each}
        </div>
        <div class="anatomy-legend">
          {#each quantization as q (q.dtype)}
            {@const pct = (q.bytes / composition.tensor_data_size) * 100}
            <span class="key"><span class="swatch quant-{q.colorClass}"></span>{q.dtype} — {formatBytes(q.bytes)} ({pct.toFixed(1)}%)</span>
          {/each}
        </div>
      </section>
    {/if}

    <section>
      <h3>Metadata <span class="hint">{composition.metadata.length} entries</span></h3>
      <div class="table-wrap">
        <table>
          <thead><tr><th>Key</th><th>Value</th></tr></thead>
          <tbody>
            {#each composition.metadata as entry (entry.key)}
              <tr>
                <td class="key-cell">{entry.key}</td>
                <td class="value-cell">{entry.value}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </section>

    <section>
      <h3>Tensors <span class="hint">{composition.tensors.length} total, largest first</span></h3>
      <div class="table-wrap">
        <table>
          <thead><tr><th>Name</th><th>Shape</th><th>Dtype</th><th class="num">Size</th></tr></thead>
          <tbody>
            {#each composition.tensors as tensor (tensor.name)}
              <tr>
                <td class="key-cell">{tensor.name}</td>
                <td class="value-cell">{tensor.shape.join(" × ")}</td>
                <td class="value-cell">{tensor.dtype}</td>
                <td class="size-cell">
                  <div class="meter"><div class="fill" style="width: {(tensor.byte_size / maxTensorBytes) * 100}%"></div></div>
                  <span class="size-value">{formatBytes(tensor.byte_size)}</span>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </section>
  {:else if !loading}
    <div class="idle"><span>Click "Inspect GGUF file" to read its anatomy, metadata, and tensors</span></div>
  {/if}
</div>

<style>
  .inspector {
    display: flex;
    flex-direction: column;
    gap: 1.2rem;
  }
  .controls {
    display: flex;
  }
  .controls button {
    font-size: 0.8rem;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  h3 {
    margin: 0;
    font-size: 0.85rem;
    color: var(--ink);
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
  }
  .hint {
    font-size: 0.72rem;
    font-weight: 400;
    color: var(--ink-muted);
  }
  .about {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    padding: 0.9em 1em;
    border-radius: 10px;
    background: var(--surface-raised);
  }
  .about-title {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
    font-size: 0.95rem;
    color: var(--ink);
    font-weight: 600;
  }
  .badge {
    font-size: 0.68rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    padding: 0.25em 0.6em;
    border-radius: 999px;
    background: var(--surface);
    color: var(--ink-muted);
    border: 1px solid var(--baseline);
  }
  .badge.chat {
    color: var(--signal);
    border-color: var(--signal);
  }
  .about-facts {
    display: flex;
    gap: 0.5rem 1.2rem;
    flex-wrap: wrap;
    font-size: 0.78rem;
    color: var(--ink-muted);
  }
  .about-facts strong {
    color: var(--ink);
    font-weight: 600;
    margin-right: 0.3em;
  }
  .about-note {
    margin: 0;
    font-size: 0.75rem;
    color: var(--ink-muted);
  }
  .special-tokens {
    display: flex;
    gap: 0.5rem 1rem;
    flex-wrap: wrap;
    font-size: 0.78rem;
    color: var(--ink-muted);
  }
  .token-chip strong {
    color: var(--ink);
    font-weight: 600;
    margin-right: 0.3em;
  }
  .token-chip code {
    font-family: var(--font-mono);
    background: var(--surface);
    padding: 0.1em 0.4em;
    border-radius: 4px;
  }
  .chat-template summary {
    font-size: 0.78rem;
    color: var(--ink-muted);
    cursor: pointer;
  }
  .chat-template pre {
    margin: 0.5em 0 0;
    padding: 0.7em 0.9em;
    border-radius: 8px;
    background: var(--surface);
    border: 1px solid var(--baseline);
    font-family: var(--font-mono);
    font-size: 0.72rem;
    color: var(--ink);
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 260px;
    overflow-y: auto;
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
  .status-msg.error {
    color: var(--critical);
    font-size: 0.85rem;
  }
  .anatomy-bar {
    display: flex;
    height: 22px;
    border-radius: 6px;
    overflow: hidden;
    border: 1px solid var(--baseline);
  }
  .segment {
    height: 100%;
  }
  .segment.header {
    background: var(--critical);
  }
  .segment.meta {
    background: var(--accent-2);
  }
  .segment.data {
    background: var(--signal);
  }
  .anatomy-legend {
    display: flex;
    gap: 1rem;
    flex-wrap: wrap;
    font-size: 0.72rem;
    color: var(--ink-muted);
  }
  .anatomy-legend .key {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }
  .swatch {
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }
  .swatch.header {
    background: var(--critical);
  }
  .swatch.meta {
    background: var(--accent-2);
  }
  .swatch.data {
    background: var(--signal);
  }
  .segment.quant-signal,
  .swatch.quant-signal {
    background: var(--signal);
  }
  .segment.quant-accent-2,
  .swatch.quant-accent-2 {
    background: var(--accent-2);
  }
  .segment.quant-good,
  .swatch.quant-good {
    background: var(--good);
  }
  .segment.quant-critical,
  .swatch.quant-critical {
    background: var(--critical);
  }
  .compare-bars {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .compare-row {
    display: grid;
    grid-template-columns: 11rem 1fr 5.5rem;
    align-items: center;
    gap: 0.6rem;
    font-size: 0.78rem;
    color: var(--ink-muted);
  }
  .compare-track {
    height: 14px;
    border-radius: 4px;
    background: var(--surface);
    overflow: hidden;
  }
  .compare-fill {
    height: 100%;
    border-radius: 4px;
  }
  .compare-value {
    font-family: var(--font-mono);
    color: var(--ink);
    text-align: right;
  }
  .table-wrap {
    max-height: 260px;
    overflow-y: auto;
    overflow-x: auto;
    border: 1px solid var(--baseline);
    border-radius: 8px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.8rem;
  }
  th,
  td {
    text-align: left;
    padding: 0.32em 0.6em;
    white-space: nowrap;
  }
  th {
    position: sticky;
    top: 0;
    background: var(--surface);
    color: var(--ink-muted);
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    font-weight: 600;
  }
  th.num {
    text-align: right;
  }
  .key-cell {
    font-family: var(--font-mono);
    color: var(--ink);
  }
  .value-cell {
    color: var(--ink-muted);
  }
  .size-cell {
    display: flex;
    align-items: center;
    gap: 0.5em;
    min-width: 140px;
  }
  .meter {
    flex: 1;
    min-width: 60px;
    height: 6px;
    border-radius: 3px;
    background: var(--surface-raised);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--signal);
  }
  .size-value {
    font-family: var(--font-mono);
    color: var(--ink-muted);
    width: 3.6rem;
    text-align: right;
  }
</style>
