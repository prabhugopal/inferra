<script lang="ts">
  import { history } from "./history.svelte";
  import { settings } from "./settings.svelte";

  function truncate(text: string, max: number): string {
    const flat = text.replace(/\s+/g, " ").trim();
    return flat.length > max ? `${flat.slice(0, max - 1)}…` : flat;
  }

  let recent = $derived(history.records.slice(-settings.historySize));
  // Each row keeps its true chronological run number regardless of display
  // order, so "newest first" just reverses layout, not the numbering.
  let rows = $derived(
    settings.newestFirst
      ? recent.map((record, i) => ({ record, num: i + 1 })).reverse()
      : recent.map((record, i) => ({ record, num: i + 1 })),
  );

  let scrollEl: HTMLDivElement | undefined = $state();
  // New rows append on the "newest" edge — bottom when chronological, top
  // when newest-first — keep that edge in view as history grows.
  $effect(() => {
    rows.length;
    scrollEl?.scrollTo({ top: settings.newestFirst ? 0 : scrollEl.scrollHeight });
  });
</script>

<figure class="log">
  <figcaption>Recent generations — what each chart bar actually ran</figcaption>
  {#if rows.length > 0}
    <div class="table-wrap" bind:this={scrollEl}>
      <table>
        <thead>
          <tr>
            <th class="idx">#</th>
            <th>Model</th>
            <th>Prompt</th>
            <th>Response</th>
            <th class="num">Decode</th>
          </tr>
        </thead>
        <tbody>
          {#each rows as { record, num } (record.id)}
            {@const isCurrent = num === recent.length}
            <tr class:current={isCurrent}>
              <td class="idx">{num}</td>
              <td class="model" title={record.modelId ?? "unknown model"}>{record.modelId ?? "—"}</td>
              <td class="snippet" title={record.prompt}>{truncate(record.prompt, 60)}</td>
              <td class="snippet" title={record.responseText}>{truncate(record.responseText, 60)}</td>
              <td class="num">{record.completionTokensPerSec != null ? `${record.completionTokensPerSec.toFixed(1)}/s` : "—"}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {:else}
    <div class="idle">
      <span>No generations yet</span>
    </div>
  {/if}
</figure>

<style>
  .log {
    margin: 0;
  }
  figcaption {
    font-size: 0.75rem;
    color: var(--ink-muted);
    margin-bottom: 0.4rem;
  }
  .idle {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 60px;
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
    padding: 0.35em 0.6em;
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
  th.num,
  td.num {
    text-align: right;
    font-family: var(--font-mono);
    color: var(--ink-muted);
    white-space: nowrap;
  }
  td.snippet {
    color: var(--ink);
    max-width: 22rem;
  }
  tr.current td {
    color: var(--ink);
    font-weight: 600;
  }
  tr.current td.idx {
    color: var(--signal);
  }
</style>
