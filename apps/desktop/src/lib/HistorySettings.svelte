<script lang="ts">
  import { settings, updateSettings } from "./settings.svelte";

  function onSizeInput(event: Event): void {
    const raw = Number((event.target as HTMLInputElement).value);
    updateSettings({ historySize: Number.isFinite(raw) ? Math.min(50, Math.max(1, Math.round(raw))) : settings.historySize });
  }
</script>

<details class="settings">
  <summary>History settings</summary>
  <div class="fields">
    <label>
      Keep last
      <input type="number" min="1" max="50" value={settings.historySize} oninput={onSizeInput} />
      generations
    </label>
    <label class="checkbox">
      <input
        type="checkbox"
        checked={settings.newestFirst}
        onchange={(event) => updateSettings({ newestFirst: (event.target as HTMLInputElement).checked })}
      />
      Show newest first
    </label>
  </div>
</details>

<style>
  .settings summary {
    font-size: 0.78rem;
    color: var(--ink-muted);
    cursor: pointer;
  }
  .fields {
    display: flex;
    align-items: center;
    gap: 1.2rem;
    flex-wrap: wrap;
    margin-top: 0.6rem;
  }
  label {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.78rem;
    color: var(--ink-muted);
  }
  label.checkbox {
    gap: 0.45rem;
  }
  input[type="number"] {
    width: 3.5rem;
    font-family: var(--font-mono);
    font-size: 0.78rem;
  }
</style>
