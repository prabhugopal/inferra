<script lang="ts">
  import { history } from "./history.svelte";
  import { settings } from "./settings.svelte";

  const BAR_W = 24;
  const GAP = 4;
  const HEIGHT = 160;
  const TOP_PAD = 20;
  const SEGMENT_GAP = 2;
  const AXIS_H = 20;

  interface Bar {
    id: number;
    prefillSec: number;
    decodeSec: number;
  }

  let recent = $derived(
    history.records
      .filter((r) => r.promptTimeSec != null && r.completionTimeSec != null)
      .map((r) => ({ id: r.id, prefillSec: r.promptTimeSec!, decodeSec: r.completionTimeSec! }))
      .slice(-settings.historySize) as Bar[],
  );
  // Each bar keeps its true chronological run number regardless of display
  // order, so "newest first" just reverses layout, not the numbering.
  let bars = $derived(
    settings.newestFirst
      ? recent.map((bar, i) => ({ bar, num: i + 1 })).reverse()
      : recent.map((bar, i) => ({ bar, num: i + 1 })),
  );
  let maxTotal = $derived(Math.max(0.001, ...recent.map((b) => b.prefillSec + b.decodeSec)));
  let width = $derived(Math.max(1, bars.length) * (BAR_W + GAP));

  let scrollEl: HTMLDivElement | undefined = $state();
  // New bars append on the "newest" edge — right when chronological,
  // left when newest-first — keep that edge in view as history grows.
  $effect(() => {
    bars.length;
    scrollEl?.scrollTo({ left: settings.newestFirst ? 0 : scrollEl.scrollWidth });
  });
</script>

<figure class="latency">
  <figcaption>Latency breakdown by generation (seconds, from mistral.rs)</figcaption>
  {#if bars.length > 0}
    <div class="scroll" bind:this={scrollEl}>
      <svg viewBox="0 0 {width} {HEIGHT + AXIS_H}" height={HEIGHT + AXIS_H} width={width}>
        <line x1="0" y1={HEIGHT - 0.5} x2={width} y2={HEIGHT - 0.5} class="baseline" />
        {#each bars as { bar, num }, i (bar.id)}
          {@const total = bar.prefillSec + bar.decodeSec}
          {@const rawDecodeHeight = ((HEIGHT - TOP_PAD) * bar.decodeSec) / maxTotal}
          {@const rawPrefillHeight = ((HEIGHT - TOP_PAD) * bar.prefillSec) / maxTotal}
          {@const decodeHeight = Math.max(rawDecodeHeight - SEGMENT_GAP / 2, 2)}
          {@const prefillHeight = Math.max(rawPrefillHeight - SEGMENT_GAP / 2, 0)}
          {@const x = i * (BAR_W + GAP)}
          {@const isCurrent = num === recent.length}
          <g>
            <rect
              x={x}
              y={HEIGHT - decodeHeight}
              width={BAR_W}
              height={Math.max(decodeHeight, 2)}
              rx="4"
              class="decode"
              class:current={isCurrent}
            >
              <title>decode: {bar.decodeSec.toFixed(2)}s{isCurrent ? " — latest" : ""}</title>
            </rect>
            <rect
              x={x}
              y={HEIGHT - decodeHeight - SEGMENT_GAP - prefillHeight}
              width={BAR_W}
              height={prefillHeight}
              rx="4"
              class="prefill"
              class:current={isCurrent}
            >
              <title>prefill: {bar.prefillSec.toFixed(2)}s{isCurrent ? " — latest" : ""}</title>
            </rect>
            <text x={x + BAR_W / 2} y={HEIGHT - decodeHeight - SEGMENT_GAP - prefillHeight - 4} class="value-label">
              {total.toFixed(1)}s
            </text>
            <text x={x + BAR_W / 2} y={HEIGHT + 13} class="axis-label" class:current={isCurrent}>{num}</text>
          </g>
        {/each}
      </svg>
    </div>
    <div class="legend">
      <span class="key"><span class="swatch prefill"></span>Prefill</span>
      <span class="key"><span class="swatch decode"></span>Decode</span>
    </div>
  {:else}
    <div class="idle" style="height: {HEIGHT}px">
      <span>No generations yet</span>
    </div>
    <div class="legend">
      <span class="key"><span class="swatch prefill"></span>Prefill</span>
      <span class="key"><span class="swatch decode"></span>Decode</span>
    </div>
  {/if}
</figure>

<style>
  .latency {
    margin: 0;
  }
  .scroll {
    overflow-x: auto;
  }
  .idle {
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px dashed var(--baseline);
    border-radius: 8px;
    color: var(--ink-muted);
    font-size: 0.78rem;
  }
  figcaption {
    font-size: 0.75rem;
    color: var(--ink-muted);
    margin-bottom: 0.4rem;
  }
  .baseline {
    stroke: var(--baseline);
    stroke-width: 1;
  }
  .prefill {
    fill: var(--signal);
  }
  .decode {
    fill: var(--accent-2);
  }
  .prefill.current,
  .decode.current {
    stroke: var(--ink);
    stroke-width: 2;
  }
  .value-label {
    font-size: 10px;
    fill: var(--ink-muted);
    text-anchor: middle;
  }
  .axis-label {
    font-size: 9px;
    fill: var(--ink-muted);
    text-anchor: middle;
  }
  .axis-label.current {
    fill: var(--ink);
    font-weight: 700;
  }
  .legend {
    display: flex;
    gap: 1rem;
    margin-top: 0.4rem;
  }
  .key {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.72rem;
    color: var(--ink-muted);
  }
  .swatch {
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }
  .swatch.prefill {
    background: var(--signal);
  }
  .swatch.decode {
    background: var(--accent-2);
  }
</style>
