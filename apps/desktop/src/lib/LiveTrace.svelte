<script lang="ts">
  import { live } from "./live.svelte";
  import { confidence } from "./logprobs.svelte";
  import { modelState } from "./state.svelte";
  import { formatBytes } from "./format";

  let last = $derived(live.points.at(-1) ?? null);
  // From the model's real GGUF metadata (layers/heads/head-dim), fetched
  // once at load time — null only if that read failed for this model.
  let bytesPerToken = $derived(modelState.kvCache?.bytes_per_token ?? null);

  // Prefill (the prompt) plus every token streamed so far — not just the
  // streamed count alone, since the cache holds both.
  function cacheBytesAt(tokensSoFar: number): number | null {
    return bytesPerToken != null ? (live.promptTokens + tokensSoFar) * bytesPerToken : null;
  }

  let scrollEl: HTMLDivElement | undefined = $state();
  $effect(() => {
    live.points.length;
    scrollEl?.scrollTo({ top: scrollEl.scrollHeight });
  });

  function pct(p: number): string {
    return `${(p * 100).toFixed(1)}%`;
  }
</script>

<figure class="live">
  <figcaption>Live · tokens streamed vs. time (real arrival times)</figcaption>
  {#if live.points.length > 0 && last}
    <p class="readout">
      {live.points.length} / {live.budget} tokens · {(last.ms / 1000).toFixed(1)}s so far
      {#if cacheBytesAt(last.tokens) != null}
        · KV cache ~{formatBytes(cacheBytesAt(last.tokens)!)}
      {/if}
    </p>
    {#if confidence.wasGreedy}
      <p class="note">
        Confidence unavailable at temperature 0 — mistral.rs's greedy sampling path doesn't report true
        log-probabilities. Try temperature &gt; 0.
      </p>
    {/if}
    <div class="table-wrap" bind:this={scrollEl}>
      <table>
        <thead>
          <tr>
            <th class="idx">#</th>
            <th>Token</th>
            <th class="num">Gap</th>
            {#if bytesPerToken != null}<th class="num">KV cache</th>{/if}
            <th>Confidence</th>
          </tr>
        </thead>
        <tbody>
          {#each live.points as point, i (point.tokens)}
            {@const isCurrent = i === live.points.length - 1}
            {@const entry = confidence.entries[i]}
            <tr class:current={isCurrent}>
              <td class="idx">{point.tokens}</td>
              <td class="token">{JSON.stringify(point.text)}</td>
              <td class="num" title={i === 0 ? "includes prefill (time to first token)" : "time since previous token"}>
                +{point.gapMs.toFixed(0)}ms
              </td>
              {#if bytesPerToken != null}
                <td class="num" title="{live.promptTokens} prompt + {point.tokens} generated so far">
                  {formatBytes(cacheBytesAt(point.tokens)!)}
                </td>
              {/if}
              <td class="prob">
                {#if entry}
                  <div class="meter"><div class="fill" style="width: {Math.min(100, entry.probability * 100)}%"></div></div>
                  <span class="prob-value">{pct(entry.probability)}</span>
                {:else}
                  <span class="prob-na">—</span>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {:else}
    <div class="idle">
      <span>{live.active ? "Waiting for the first token…" : "Generate to see live token arrivals"}</span>
    </div>
    <p class="readout">&nbsp;</p>
  {/if}
</figure>

<style>
  .live {
    margin: 0;
  }
  figcaption {
    font-size: 0.75rem;
    color: var(--ink-muted);
    margin-bottom: 0.3rem;
  }
  .readout {
    margin: 0 0 0.4rem;
    font-size: 0.72rem;
    color: var(--ink-muted);
    font-family: var(--font-mono);
  }
  .note {
    margin: 0 0 0.4rem;
    font-size: 0.75rem;
    color: var(--ink-muted);
  }
  .idle {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 90px;
    padding: 0.6em 1.2em;
    border: 1px dashed var(--baseline);
    border-radius: 8px;
    color: var(--ink-muted);
    font-size: 0.78rem;
  }
  .table-wrap {
    max-height: 220px;
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
    padding: 0.3em 0.6em;
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
  td.idx,
  th.idx {
    color: var(--ink-muted);
    font-family: var(--font-mono);
    width: 2rem;
  }
  td.token {
    font-family: var(--font-mono);
    color: var(--ink);
    white-space: nowrap;
  }
  th.num,
  td.num {
    text-align: right;
    font-family: var(--font-mono);
    color: var(--ink-muted);
    white-space: nowrap;
  }
  td.prob {
    display: flex;
    align-items: center;
    gap: 0.5em;
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
  .prob-value {
    font-family: var(--font-mono);
    color: var(--ink-muted);
    width: 3.2rem;
    text-align: right;
  }
  .prob-na {
    color: var(--ink-muted);
    font-family: var(--font-mono);
  }
  tr.current td {
    color: var(--ink);
    font-weight: 600;
  }
  tr.current td.idx {
    color: var(--signal);
  }
</style>
