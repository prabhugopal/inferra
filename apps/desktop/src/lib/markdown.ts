// Deliberately not a full markdown renderer: splits fenced code blocks out
// of the rest of the text so they can be styled differently. Everything
// still goes through plain Svelte text interpolation (never {@html}), so
// there's no HTML-injection surface even though this text comes from the
// model's own output.
export interface TextSegment {
  type: "text";
  content: string;
}

export interface CodeSegment {
  type: "code";
  lang: string;
  content: string;
}

export type MarkdownSegment = TextSegment | CodeSegment;

const FENCE_RE = /```(\w*)\n?([\s\S]*?)```/g;

export function parseSegments(source: string): MarkdownSegment[] {
  const segments: MarkdownSegment[] = [];
  let lastIndex = 0;

  for (const match of source.matchAll(FENCE_RE)) {
    const [full, lang, code] = match;
    const start = match.index ?? 0;
    if (start > lastIndex) {
      segments.push({ type: "text", content: source.slice(lastIndex, start) });
    }
    segments.push({ type: "code", lang: lang || "text", content: code.replace(/\n$/, "") });
    lastIndex = start + full.length;
  }

  if (lastIndex < source.length) {
    segments.push({ type: "text", content: source.slice(lastIndex) });
  }
  if (segments.length === 0) {
    segments.push({ type: "text", content: source });
  }
  return segments;
}
