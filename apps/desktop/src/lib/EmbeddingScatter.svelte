<script lang="ts">
  import Plotly from "plotly.js-dist-min";
  import { modelState } from "./state.svelte";
  import { sampleGgufEmbeddings, sampleGgufEmbeddingsNear, tokenizeText, type EmbeddingPoint } from "./api";

  let seedText = $state("");
  let sampleSize = $state(500);
  let points = $state<EmbeddingPoint[] | null>(null);
  let loading = $state(false);
  let error = $state("");
  let plotEl: HTMLDivElement | undefined = $state();

  // Without this, switching models leaves the previous model's embedding
  // plot on screen with nothing to mark it stale — easy to mistake for the
  // newly loaded model's data. Any change in which model is active clears it
  // back to the empty state, so a fresh "Compute" is required per model.
  let lastModelKey = modelKey();
  $effect(() => {
    const key = modelKey();
    if (key !== lastModelKey) {
      lastModelKey = key;
      points = null;
      error = "";
    }
  });

  function modelKey(): string | null {
    return modelState.modelDir && modelState.ggufFile ? `${modelState.modelDir}::${modelState.ggufFile}` : null;
  }

  async function compute() {
    const dir = modelState.modelDir;
    const file = modelState.ggufFile;
    if (!dir || !file) return;
    loading = true;
    error = "";
    try {
      const text = seedText.trim();
      if (text) {
        const seedTokenIds = await tokenizeText(text);
        if (seedTokenIds.length === 0) throw new Error("that text produced no tokens");
        points = await sampleGgufEmbeddingsNear(dir, file, seedTokenIds, undefined, sampleSize);
      } else {
        points = await sampleGgufEmbeddings(dir, file, undefined, sampleSize);
      }
    } catch (err) {
      error = String(err);
      points = null;
    } finally {
      loading = false;
    }
  }

  function cssVar(name: string): string {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  }

  let seedCount = $derived(points?.filter((p) => p.is_seed).length ?? 0);

  $effect(() => {
    if (!points || !plotEl) return;
    const ink = cssVar("--ink");
    const inkMuted = cssVar("--ink-muted");
    const signal = cssVar("--signal");
    const accent2 = cssVar("--accent-2");
    const baseline = cssVar("--baseline");
    const surfaceRaised = cssVar("--surface-raised");

    function trace(subset: EmbeddingPoint[], name: string, color: string, size: number): Partial<Plotly.ScatterData> {
      return {
        type: "scatter3d",
        mode: "markers",
        name,
        x: subset.map((p) => p.x),
        y: subset.map((p) => p.y),
        z: subset.map((p) => p.z),
        text: subset.map((p) => p.token),
        hovertemplate: "%{text}<extra></extra>",
        marker: { size, color, opacity: 0.85 },
      };
    }

    const traces: Partial<Plotly.ScatterData>[] =
      seedCount > 0
        ? [
            trace(points.filter((p) => !p.is_seed), "Related", accent2, 3),
            trace(points.filter((p) => p.is_seed), "Your text", signal, 6),
          ]
        : [trace(points, "Sampled", signal, 3.5)];

    const axis = {
      title: { text: "" },
      color: inkMuted,
      gridcolor: baseline,
      zerolinecolor: baseline,
      backgroundcolor: "transparent",
    };

    const layout: Partial<Plotly.Layout> = {
      paper_bgcolor: "transparent",
      plot_bgcolor: "transparent",
      margin: { l: 0, r: 0, t: 0, b: 0 },
      font: { color: inkMuted, size: 10 },
      showlegend: seedCount > 0,
      legend: { font: { color: inkMuted }, bgcolor: "transparent" },
      hoverlabel: { bgcolor: surfaceRaised, bordercolor: baseline, font: { color: ink, size: 11 } },
      hovermode: "closest",
      scene: {
        xaxis: { ...axis, title: { text: "PC1" } },
        yaxis: { ...axis, title: { text: "PC2" } },
        zaxis: { ...axis, title: { text: "PC3" } },
      },
    };

    Plotly.newPlot(plotEl, traces, layout, {
      // "hover" keeps the toolbar out of the way at rest instead of a
      // permanent light-gray strip sitting over a dark plot.
      displayModeBar: "hover",
      displaylogo: false,
      modeBarButtonsToRemove: ["resetCameraLastSave3d", "tableRotation", "orbitRotation"],
      responsive: true,
    });

    return () => {
      if (plotEl) Plotly.purge(plotEl);
    };
  });
</script>

<div class="scatter">
  <div class="controls">
    <label class="grow">
      Seed text <span class="hint">optional — leave blank for an even sample across the whole vocabulary</span>
      <input type="text" placeholder="e.g. king queen throne crown" bind:value={seedText} />
    </label>
    <label class="inline">
      Sample
      <input type="number" min="10" max="3000" step="10" bind:value={sampleSize} />
      tokens
    </label>
    <button type="button" onclick={compute} disabled={!modelState.modelDir || loading}>
      {loading ? "Computing…" : "Compute embedding clusters"}
    </button>
  </div>

  {#if error}
    <p class="status-msg error">{error}</p>
  {/if}

  {#if !points && !loading}
    <div class="idle">
      <span>
        {modelState.modelDir
          ? "Sample and project the token embedding space into 3D — with seed text, see what actually clusters around your words; without it, see a cross-section of the whole vocabulary."
          : "Load a model first."}
      </span>
    </div>
  {:else}
    <div class="plot" bind:this={plotEl}></div>
    {#if points}
      <p class="readout">
        {points.length} tokens
        {#if seedCount > 0}· {seedCount} from your text, {points.length - seedCount} nearest neighbors{/if}
        · PCA-reduced from the model's own embedding matrix — drag to rotate, scroll to zoom
      </p>
    {/if}
  {/if}
</div>

<style>
  .scatter {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }
  .controls {
    display: flex;
    align-items: flex-end;
    gap: 1rem;
    flex-wrap: wrap;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    font-size: 0.78rem;
    color: var(--ink-muted);
  }
  label.inline {
    flex-direction: row;
    align-items: center;
    gap: 0.5rem;
    white-space: nowrap;
  }
  label.grow {
    flex: 1;
    min-width: 220px;
  }
  .hint {
    font-weight: 400;
    opacity: 0.8;
  }
  input[type="text"] {
    font-size: 0.85rem;
    padding: 0.4em 0.6em;
    border-radius: 8px;
    background: var(--surface-raised);
    color: var(--ink);
    border: 1px solid transparent;
  }
  input[type="number"] {
    width: 5rem;
    font-family: var(--font-mono);
    font-size: 0.78rem;
  }
  button {
    font-size: 0.8rem;
  }
  .status-msg.error {
    color: var(--critical);
    font-size: 0.85rem;
  }
  .idle {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 200px;
    padding: 0.6em 1.2em;
    text-align: center;
    border: 1px dashed var(--baseline);
    border-radius: 8px;
    color: var(--ink-muted);
    font-size: 0.78rem;
  }
  .plot {
    width: 100%;
    height: 420px;
    border: 1px solid var(--baseline);
    border-radius: 8px;
  }
  .readout {
    margin: 0;
    font-size: 0.72rem;
    color: var(--ink-muted);
    font-family: var(--font-mono);
  }
</style>
