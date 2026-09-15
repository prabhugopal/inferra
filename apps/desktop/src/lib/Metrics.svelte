<script lang="ts">
  import { history } from "./history.svelte";
  import { formatSeconds, formatTokensPerSec, formatTokenCount } from "./format";

  let latest = $derived(history.records.at(-1) ?? null);
</script>

<div class="stat-tiles">
  <div class="tile">
    <span class="label">Elapsed</span>
    <span class="value">{latest ? formatSeconds(latest.elapsedMs) : "—"}</span>
  </div>
  <div class="tile">
    <span class="label">Prefill</span>
    <span class="value">{formatTokensPerSec(latest?.promptTokensPerSec ?? null)}</span>
  </div>
  <div class="tile">
    <span class="label">Decode</span>
    <span class="value">{formatTokensPerSec(latest?.completionTokensPerSec ?? null)}</span>
  </div>
  <div class="tile">
    <span class="label">Prompt tokens</span>
    <span class="value">{formatTokenCount(latest?.promptTokens ?? null)}</span>
  </div>
  <div class="tile">
    <span class="label">Completion tokens</span>
    <span class="value">{formatTokenCount(latest?.completionTokens ?? null)}</span>
  </div>
</div>

<style>
  .stat-tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
    gap: 2px;
    background: var(--surface);
    border-radius: 10px;
    overflow: hidden;
    align-self: stretch;
  }
  .tile {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 0.35rem;
    padding: 1.1rem 0.9rem;
    background: var(--surface-raised);
  }
  .label {
    font-size: 0.75rem;
    color: var(--ink-muted);
  }
  .value {
    font-size: 1.5rem;
    font-weight: 600;
    color: var(--ink);
  }
</style>
