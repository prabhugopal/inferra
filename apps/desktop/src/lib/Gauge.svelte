<script lang="ts">
  import { history } from "./history.svelte";

  const R = 42;
  const SIZE = 150;
  const CIRCUMFERENCE = 2 * Math.PI * R;

  let decodeValues = $derived(
    history.records.map((r) => r.completionTokensPerSec).filter((v): v is number => v != null),
  );
  let latest = $derived(decodeValues.at(-1) ?? null);
  // Relative to this session's own peak — an honest scale, not a fabricated target.
  let peak = $derived(Math.max(1, ...decodeValues));
  let ratio = $derived(latest != null ? Math.min(1, latest / peak) : 0);
  let dashOffset = $derived(CIRCUMFERENCE * (1 - ratio));
</script>

<div class="gauge-panel">
  <div class="gauge" style="width: {SIZE}px; height: {SIZE}px">
    <svg viewBox="0 0 100 100" width={SIZE} height={SIZE}>
      <circle class="track" cx="50" cy="50" r={R} />
      {#if latest != null}
        <circle
          class="fill"
          cx="50"
          cy="50"
          r={R}
          stroke-dasharray={CIRCUMFERENCE}
          stroke-dashoffset={dashOffset}
          transform="rotate(-90 50 50)"
        />
      {/if}
    </svg>
    <div class="readout">
      <span class="num">{latest != null ? latest.toFixed(1) : "—"}</span>
      <span class="unit">tok/s</span>
    </div>
  </div>
  <p class="caption">
    {#if latest != null}
      Decode throughput · {Math.round(ratio * 100)}% of session peak ({peak.toFixed(1)})
    {:else}
      Decode throughput · no generations yet this session
    {/if}
  </p>
</div>

<style>
  .gauge-panel {
    display: flex;
    flex-direction: column;
    align-items: center;
    flex-shrink: 0;
  }
  .gauge {
    position: relative;
  }
  .track {
    fill: none;
    stroke: var(--signal-track);
    stroke-width: 7;
  }
  .fill {
    fill: none;
    stroke: var(--signal);
    stroke-width: 7;
    stroke-linecap: round;
    transition: stroke-dashoffset 0.4s ease;
  }
  .readout {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
  }
  .num {
    font-size: 2rem;
    font-weight: 600;
    color: var(--ink);
  }
  .unit {
    font-size: 0.8rem;
    color: var(--ink-muted);
  }
  .caption {
    margin: 0.6rem 0 0;
    font-size: 0.8rem;
    color: var(--ink-muted);
  }
</style>
