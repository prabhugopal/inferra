<script lang="ts">
  import { history } from "./history.svelte";
  import { settings } from "./settings.svelte";

  const BAR_W = 24;
  const GAP = 4;
  const HEIGHT = 160;
  const TOP_PAD = 24;
  const AXIS_H = 20;

  let recent = $derived(
    history.records
      .filter((record) => record.completionTokensPerSec != null)
      .slice(-settings.historySize) as (typeof history.records[number] & {
      completionTokensPerSec: number;
    })[],
  );
  // Each bar keeps its true chronological run number regardless of display
  // order, so "newest first" just reverses layout, not the numbering.
  let bars = $derived(
    settings.newestFirst
      ? recent.map((bar, i) => ({ bar, num: i + 1 })).reverse()
      : recent.map((bar, i) => ({ bar, num: i + 1 })),
  );
  let maxValue = $derived(Math.max(1, ...recent.map((record) => record.completionTokensPerSec)));
  let width = $derived(Math.max(1, bars.length) * (BAR_W + GAP));

  let scrollEl: HTMLDivElement | undefined = $state();
  // New bars append on the "newest" edge — right when chronological,
  // left when newest-first — keep that edge in view as history grows.
  $effect(() => {
    bars.length;
    scrollEl?.scrollTo({ left: settings.newestFirst ? 0 : scrollEl.scrollWidth });
  });
</script>

<figure class="history">
  <figcaption>Decode throughput by generation (tokens/sec, from mistral.rs)</figcaption>
  {#if bars.length > 0}
    <div class="scroll" bind:this={scrollEl}>
      <svg viewBox="0 0 {width} {HEIGHT + AXIS_H}" height={HEIGHT + AXIS_H} width={width}>
        <line x1="0" y1={HEIGHT - 0.5} x2={width} y2={HEIGHT - 0.5} class="baseline" />
        {#each bars as { bar, num }, i (bar.id)}
          {@const barHeight = ((HEIGHT - TOP_PAD) * bar.completionTokensPerSec) / maxValue}
          {@const x = i * (BAR_W + GAP)}
          {@const isCurrent = num === recent.length}
          <g>
            <rect
              x={x}
              y={HEIGHT - barHeight}
              width={BAR_W}
              height={Math.max(barHeight, 4)}
              rx="4"
              class="bar"
              class:current={isCurrent}
            >
              <title>{bar.completionTokensPerSec.toFixed(1)} tokens/sec (decode){isCurrent ? " — latest" : ""}</title>
            </rect>
            <text x={x + BAR_W / 2} y={HEIGHT - barHeight - 4} class="value-label">
              {bar.completionTokensPerSec.toFixed(0)}
            </text>
            <text x={x + BAR_W / 2} y={HEIGHT + 13} class="axis-label" class:current={isCurrent}>{num}</text>
          </g>
        {/each}
      </svg>
    </div>
  {:else}
    <div class="idle" style="height: {HEIGHT}px">
      <span>No generations yet</span>
    </div>
  {/if}
</figure>

<style>
  .history {
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
  .bar {
    fill: var(--signal);
  }
  .bar.current {
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
</style>
