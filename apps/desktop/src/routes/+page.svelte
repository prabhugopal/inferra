<script lang="ts">
  import ModelLoader from "$lib/ModelLoader.svelte";
  import Chat from "$lib/Chat.svelte";
  import Metrics from "$lib/Metrics.svelte";
  import HistoryChart from "$lib/HistoryChart.svelte";
  import LatencyChart from "$lib/LatencyChart.svelte";
  import Gauge from "$lib/Gauge.svelte";
  import GenerationLog from "$lib/GenerationLog.svelte";
  import HistorySettings from "$lib/HistorySettings.svelte";
  import LiveTrace from "$lib/LiveTrace.svelte";
  import GgufComposition from "$lib/GgufComposition.svelte";
  import EmbeddingScatter from "$lib/EmbeddingScatter.svelte";
  import ModelBrowser from "$lib/ModelBrowser.svelte";
  import { modelState } from "$lib/state.svelte";
  import { MessageSquare, LineChart, Microscope, Compass } from "lucide-svelte";

  const statusText: Record<typeof modelState.status, string> = {
    unloaded: "No model loaded",
    loading: "Loading model…",
    ready: "Ready",
    error: "Load failed",
  };

  let activeTab = $state<"chat" | "metrics" | "inspector" | "browse">("chat");
</script>

<div class="shell">
  <header>
    <div class="brand">
      <img src="/logo.png" alt="" width="28" height="28" />
      <h1>Inferra</h1>
    </div>
    <div class="status" data-state={modelState.status}>
      <span class="dot"></span>
      {statusText[modelState.status]}
      {#if modelState.status === "ready" && modelState.info}
        <span class="model-name">· {modelState.info.model_id}</span>
      {/if}
    </div>
  </header>

  <div class="body">
    <ModelLoader />
    <div class="content">
      <nav class="tabs">
        <button type="button" class:active={activeTab === "chat"} onclick={() => (activeTab = "chat")}>
          <MessageSquare size={15} aria-hidden="true" />
          Chat
        </button>
        <button type="button" class:active={activeTab === "metrics"} onclick={() => (activeTab = "metrics")}>
          <LineChart size={15} aria-hidden="true" />
          Session metrics
        </button>
        <button type="button" class:active={activeTab === "inspector"} onclick={() => (activeTab = "inspector")}>
          <Microscope size={15} aria-hidden="true" />
          Model Inspector
        </button>
        <button type="button" class:active={activeTab === "browse"} onclick={() => (activeTab = "browse")}>
          <Compass size={15} aria-hidden="true" />
          Browse models
        </button>
      </nav>
      <main>
        <!-- Both tabs stay mounted (hidden via CSS, not unmounted) so an
             in-flight generation's local state survives switching tabs. -->
        <section hidden={activeTab !== "chat"}>
          <Chat />
          <LiveTrace />
        </section>
        <section hidden={activeTab !== "metrics"}>
          <HistorySettings />
          <div class="charts-row">
            <HistoryChart />
            <LatencyChart />
          </div>
          <GenerationLog />
          <div class="metrics-row">
            <Gauge />
            <Metrics />
          </div>
        </section>
        <section hidden={activeTab !== "inspector"}>
          <GgufComposition />
          <EmbeddingScatter />
        </section>
        <section hidden={activeTab !== "browse"}>
          <ModelBrowser />
        </section>
      </main>
    </div>
  </div>
</div>

<style>
  :root {
    color-scheme: dark;
    --bg: #0f1420;
    --surface: #161c2c;
    --surface-raised: #1f2740;
    --ink: #eef1f8;
    --ink-muted: #8b93ac;
    --signal: #3987e5;
    --signal-track: #263352;
    --accent-2: #d95926;
    --good: #0ca30c;
    --critical: #e66767;
    --baseline: #383a4d;
    --font-mono: ui-monospace, "SF Mono", "Cascadia Code", "Roboto Mono", monospace;

    background: var(--bg);
    color: var(--ink);
    font-family: -apple-system, "Segoe UI", system-ui, sans-serif;
  }

  @media (prefers-color-scheme: light) {
    :root {
      color-scheme: light;
      --bg: #f5f6fa;
      --surface: #ffffff;
      --surface-raised: #eef1f7;
      --ink: #12141c;
      --ink-muted: #5b6178;
      --signal: #2a78d6;
      --signal-track: #cde2fb;
      --accent-2: #eb6834;
      --good: #0ca30c;
      --critical: #d03b3b;
      --baseline: #d5d8e3;
    }
  }

  .shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
    max-width: 100%;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.9rem 1.4rem;
    border-bottom: 1px solid var(--surface-raised);
    flex-shrink: 0;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 0.55rem;
  }
  .brand img {
    border-radius: 7px;
    flex-shrink: 0;
  }
  h1 {
    font-size: 1.05rem;
    font-weight: 600;
    margin: 0;
    letter-spacing: 0.01em;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.8rem;
    color: var(--ink-muted);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ink-muted);
    flex-shrink: 0;
  }
  .status[data-state="ready"] .dot {
    background: var(--good);
  }
  .status[data-state="loading"] .dot {
    background: var(--signal);
    animation: pulse 1.2s ease-in-out infinite;
  }
  .status[data-state="error"] .dot {
    background: var(--critical);
  }
  @media (prefers-reduced-motion: reduce) {
    .status[data-state="loading"] .dot {
      animation: none;
    }
  }
  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.35; }
  }
  .model-name {
    color: var(--ink);
    font-family: var(--font-mono);
  }

  .body {
    display: grid;
    grid-template-columns: 260px 1fr;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  :global(.rail) {
    border-right: 1px solid var(--surface-raised);
    padding: 1.2rem;
    overflow-y: auto;
  }

  .content {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .tabs {
    display: flex;
    gap: 0.3rem;
    padding: 0.7rem 1.4rem 0;
    flex-shrink: 0;
    border-bottom: 1px solid var(--surface-raised);
  }
  .tabs button {
    display: inline-flex;
    align-items: center;
    gap: 0.45em;
    background: transparent;
    border: none;
    border-radius: 6px 6px 0 0;
    padding: 0.5em 0.9em;
    font-size: 0.85rem;
    color: var(--ink-muted);
  }
  .tabs button:hover {
    color: var(--ink);
    filter: none;
  }
  .tabs button.active {
    color: var(--ink);
    background: var(--surface-raised);
  }

  main {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 1.4rem;
  }
  main section {
    display: flex;
    flex-direction: column;
    gap: 1.4rem;
  }
  main :global([hidden]) {
    display: none !important;
  }

  /* Each tab gets the full width, so this only wraps in genuinely tight
     windows — no horizontal scrollbar, same order preserved either way. */
  .metrics-row {
    display: flex;
    align-items: center;
    gap: 1.4rem;
    flex-wrap: wrap;
  }
  .metrics-row > :global(*:first-child) {
    flex-shrink: 0;
  }
  .metrics-row > :global(*:last-child) {
    flex: 1;
    min-width: 260px;
  }
  .charts-row {
    display: flex;
    gap: 1.4rem;
    flex-wrap: wrap;
  }
  .charts-row > :global(*) {
    flex: 1;
    min-width: 220px;
  }

  :global(button) {
    border-radius: 8px;
    border: 1px solid transparent;
    padding: 0.5em 1.1em;
    font-size: 0.85rem;
    font-weight: 500;
    cursor: pointer;
    background: var(--surface-raised);
    color: var(--ink);
  }
  :global(button:hover) {
    filter: brightness(1.1);
  }
  :global(button:disabled) {
    opacity: 0.5;
    cursor: not-allowed;
    filter: none;
  }
  :global(button:focus-visible) {
    outline: 2px solid var(--signal);
    outline-offset: 1px;
  }
  :global(input) {
    padding: 0.5em 0.6em;
    border-radius: 6px;
    border: 1px solid var(--surface-raised);
    background: var(--surface);
    color: var(--ink);
    font: inherit;
  }
  :global(input:focus-visible) {
    outline: 2px solid var(--signal);
    outline-offset: 1px;
  }
</style>
