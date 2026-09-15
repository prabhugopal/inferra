# Inferra desktop

Tauri + SvelteKit + TypeScript shell over `inferra-ui-bridge`. See the root
[README](../../README.md) for the full architecture, module map, and the
streaming data-flow diagram — this file only covers what's specific to
running/developing the desktop app itself.

`src-tauri` owns one `FrontendBridge`, built once the frontend calls
`load_model` with a local GGUF path (no model config is hardcoded — it's
supplied by the UI). Tauri commands in `src-tauri/src/lib.rs` are thin
wrappers around `FrontendBridge`; the Svelte side (`src/lib/api.ts` and
friends) never imports model-runtime concepts, only the DTOs mirrored from
`inferra-core`/`inferra-ui-bridge`. See `src/lib/` in the root README's
module map for what each file does.

## Run

```bash
bun install
bun run tauri dev
```

You'll need a local GGUF file to load from the UI (model directory + file
name); optionally a Hugging Face tokenizer repo id and/or chat template for
GGUFs that don't embed their own.

## UI shape

- **Sidebar** — model picker: native file browse (auto-loads on pick),
  recent-models list (also auto-loads; a no-op if the clicked one is already
  active), collapsed manual-config fallback.
- **Chat tab** — prompt, streamed response, and the live token-arrival trace
  for the in-flight generation.
- **Session metrics tab** — decode-throughput gauge, stat tiles, and two
  session-long bar charts (decode throughput, prefill/decode latency
  breakdown), all sourced from real `mistral.rs` `Usage` data.

Both tabs stay mounted (hidden via CSS `[hidden]`, not `{#if}`-unmounted) so
switching tabs mid-generation doesn't lose the in-flight prompt or streamed
text — that state lives in `Chat.svelte`'s local `$state`.

## Recommended IDE Setup

[VS Code](https://code.visualstudio.com/) + [Svelte](https://marketplace.visualstudio.com/items?itemName=svelte.svelte-vscode) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).
