// Per-token confidence for the current/last generation, from mistral.rs's
// own logprobs — a report, not decoration on the response text itself.
export interface ConfidenceEntry {
  token: string;
  probability: number;
}

export const confidence = $state<{ entries: ConfidenceEntry[]; wasGreedy: boolean }>({
  entries: [],
  wasGreedy: false,
});

// mistral.rs 0.8's greedy (temperature 0) sampling path doesn't report true
// log-probabilities (confirmed from its own source — see inferra-mistral's
// `valid_logprob`), so the backend sends no logprob at all in that case.
// Tracking whether *this* generation was greedy lets the UI explain an
// empty report instead of it just looking broken.
export function resetConfidence(temperature: number): void {
  confidence.entries = [];
  confidence.wasGreedy = temperature < 1e-7;
}

export function recordConfidence(token: string, logprob: number): void {
  confidence.entries = [...confidence.entries, { token, probability: Math.exp(logprob) }];
}
