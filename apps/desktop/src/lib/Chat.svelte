<script lang="ts">
  import { generateStream, tokenizeText, type GenerateView } from "./api";
  import { modelState } from "./state.svelte";
  import { recordGeneration } from "./history.svelte";
  import { startLive, recordToken, stopLive } from "./live.svelte";
  import { resetConfidence, recordConfidence } from "./logprobs.svelte";
  import MarkdownText from "./MarkdownText.svelte";

  let prompt = $state("");
  let maxTokens = $state(128);
  let temperature = $state(0);
  let topP = $state(0.95);
  let busy = $state(false);
  let streamedText = $state("");
  let result = $state<GenerateView | null>(null);
  let error = $state("");

  let ready = $derived(modelState.status === "ready");

  async function submit(event: Event) {
    event.preventDefault();
    busy = true;
    error = "";
    streamedText = "";
    result = null;
    const startedAt = performance.now();
    // Real tokenizer count, not an estimate — mistral.rs's own streaming
    // usage stats only arrive on the terminal event, too late for a live
    // KV-cache number that needs to include prefill from the start.
    // Best-effort: a tokenize failure just means the live cache size omits
    // prefill rather than blocking generation over a non-critical readout.
    const promptTokens = await tokenizeText(prompt).catch(() => []);
    startLive(maxTokens, promptTokens.length);
    resetConfidence(temperature);
    try {
      await generateStream(
        prompt,
        { max_new_tokens: maxTokens, temperature, top_p: topP },
        (event) => {
          switch (event.type) {
            case "Token":
              streamedText += event.delta;
              recordToken(performance.now() - startedAt, event.delta);
              if (event.logprob) recordConfidence(event.logprob.token, event.logprob.logprob);
              break;
            case "Done":
              result = event.view;
              recordGeneration(prompt, event.view);
              stopLive();
              busy = false;
              break;
            case "Error":
              error = event.message;
              stopLive();
              busy = false;
              break;
          }
        },
      );
    } catch (err) {
      error = String(err);
    } finally {
      // Safety net: normally already false from the Done/Error handler above.
      // Setting it here too avoids depending on the invoke promise resolving
      // strictly after the channel's terminal message is processed.
      stopLive();
      busy = false;
    }
  }
</script>

<section class="chat">
  <form onsubmit={submit}>
    <textarea
      bind:value={prompt}
      placeholder={ready ? "Ask something…" : "Choose a model to begin"}
      rows="5"
      required
      disabled={!ready}
    ></textarea>
    <div class="row">
      <div class="options">
        <label>Max tokens <input type="number" min="1" bind:value={maxTokens} /></label>
        <label>Temperature <input type="number" min="0" max="2" step="0.05" bind:value={temperature} /></label>
        <label>Top-p <input type="number" min="0" max="1" step="0.01" bind:value={topP} /></label>
      </div>
      <button type="submit" disabled={busy || !ready}>
        {busy ? "Generating…" : "Generate"}
      </button>
    </div>
  </form>

  {#if error}
    <p class="status-msg error">{error}</p>
  {/if}
  {#if busy && streamedText}
    <article class="result">
      <MarkdownText text={streamedText} />
    </article>
  {:else if result}
    <article class="result">
      <MarkdownText text={result.text} />
    </article>
  {/if}
</section>

<style>
  .chat {
    display: flex;
    flex-direction: column;
    gap: 0.8rem;
  }
  form {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
  }
  textarea {
    font: inherit;
    font-size: 0.95rem;
    padding: 0.8em;
    border-radius: 10px;
    background: var(--surface-raised);
    color: var(--ink);
    border: 1px solid transparent;
    resize: vertical;
  }
  textarea:focus-visible {
    outline: 2px solid var(--signal);
    outline-offset: 1px;
  }
  textarea::placeholder {
    color: var(--ink-muted);
  }
  .row {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
  }
  .options {
    display: flex;
    gap: 1rem;
    flex-wrap: wrap;
  }
  .options label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.75rem;
    color: var(--ink-muted);
  }
  .options input {
    width: 5.5rem;
    font-family: var(--font-mono);
    font-size: 0.8rem;
  }
  .status-msg.error {
    color: var(--critical);
    font-size: 0.85rem;
  }
  .result {
    padding: 0.9em 1em;
    border-radius: 10px;
    background: var(--surface-raised);
    line-height: 1.55;
  }
</style>
