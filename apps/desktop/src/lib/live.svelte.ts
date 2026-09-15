// Live token-arrival trace for the in-flight generation, shared by Chat.svelte
// (writer, during streaming) and LiveTrace.svelte (reader). Each point is a
// real measured arrival time — not simulated — so `gapMs` is the actual time
// since the previous token (or since the request started, for the first one),
// not a smoothed/cumulative rate.
export interface LivePoint {
  ms: number;
  tokens: number;
  text: string;
  gapMs: number;
}

export const live = $state<{ active: boolean; points: LivePoint[]; budget: number; promptTokens: number }>({
  active: false,
  points: [],
  budget: 1,
  promptTokens: 0,
});

// `promptTokens` is the real tokenizer count for this run's prompt, not an
// estimate — mistral.rs's streaming API only reports it on the terminal
// event (too late for a live number), so callers get it themselves (the
// existing `tokenize_text` command) before generation starts. It's what lets
// a live KV-cache size include prefill, not just the tokens streamed so far.
export function startLive(budget: number, promptTokens: number): void {
  live.active = true;
  live.points = [];
  live.budget = Math.max(1, budget);
  live.promptTokens = Math.max(0, promptTokens);
}

export function recordToken(ms: number, text: string): void {
  const last = live.points.at(-1);
  const tokens = (last?.tokens ?? 0) + 1;
  const gapMs = ms - (last?.ms ?? 0);
  live.points = [...live.points, { ms, tokens, text, gapMs }];
}

export function stopLive(): void {
  live.active = false;
}
