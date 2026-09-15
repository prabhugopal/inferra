<script lang="ts">
  import { parseSegments } from "./markdown";

  let { text }: { text: string } = $props();
  let segments = $derived(parseSegments(text));
</script>

<div class="markdown">
  {#each segments as segment, i (i)}
    {#if segment.type === "code"}
      <div class="code-block">
        <div class="code-label">{segment.lang}</div>
        <pre>{segment.content}</pre>
      </div>
    {:else if segment.content.trim()}
      <p>{segment.content.trim()}</p>
    {/if}
  {/each}
</div>

<style>
  .markdown {
    display: flex;
    flex-direction: column;
    gap: 0.6em;
  }
  p {
    margin: 0;
    white-space: pre-wrap;
  }
  .code-block {
    border-radius: 8px;
    overflow: hidden;
    border: 1px solid var(--baseline);
  }
  .code-label {
    background: var(--surface);
    color: var(--ink-muted);
    font-family: var(--font-mono);
    font-size: 0.7rem;
    padding: 0.35em 0.7em;
    border-bottom: 1px solid var(--baseline);
  }
  .code-block pre {
    margin: 0;
    padding: 0.7em 0.9em;
    background: var(--surface);
    color: var(--ink);
    overflow-x: auto;
    font-family: var(--font-mono);
    font-size: 0.82rem;
    line-height: 1.5;
    white-space: pre;
  }
</style>
