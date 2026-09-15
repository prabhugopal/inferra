export function formatSeconds(ms: number): string {
  return `${(ms / 1000).toFixed(ms >= 10_000 ? 1 : 2)}s`;
}

export function formatTokensPerSec(value: number | null): string {
  if (value == null) return "—";
  return `${value.toFixed(1)}/s`;
}

export function formatTokenCount(value: number | null): string {
  if (value == null) return "—";
  return value.toLocaleString();
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value >= 10 ? 0 : 1)} ${units[unit]}`;
}

// "97.8k", "1.4M" — compact form for download/like counts.
export function formatCompactCount(value: number): string {
  if (value < 1000) return String(value);
  const units = ["k", "M", "B"];
  let scaled = value / 1000;
  let unit = 0;
  while (scaled >= 1000 && unit < units.length - 1) {
    scaled /= 1000;
    unit += 1;
  }
  return `${scaled.toFixed(scaled >= 10 ? 0 : 1)}${units[unit]}`;
}

// "2 days ago", "3 months ago" — from an ISO timestamp string.
export function formatRelativeTime(iso: string): string {
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return iso;
  const seconds = Math.max(0, (Date.now() - then) / 1000);
  const steps: [number, string][] = [
    [60, "second"],
    [60, "minute"],
    [24, "hour"],
    [30, "day"],
    [12, "month"],
    [Number.POSITIVE_INFINITY, "year"],
  ];
  let value = seconds;
  for (const [span, unit] of steps) {
    if (value < span) {
      const rounded = Math.floor(value);
      return `${rounded} ${unit}${rounded === 1 ? "" : "s"} ago`;
    }
    value /= span;
  }
  return iso;
}
