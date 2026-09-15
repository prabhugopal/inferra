// App-level preferences the user can tune, persisted per-viewer in
// localStorage — same best-effort treatment as persist.ts's "recent models".
const STORAGE_KEY = "inferra.settings";

export interface AppSettings {
  // How many recent generations the Session metrics charts/log keep and show.
  historySize: number;
  // Reading order for those charts/log: chronological (oldest -> newest,
  // left -> right, matching the live token chart) or newest-first.
  newestFirst: boolean;
  // How long a Browse-models result (search, per-repo metadata, file list)
  // stays fresh before the backend will hit Hugging Face Hub again for the
  // same request. 0 = caching off. Lives in-memory on the Rust side only —
  // this value is just passed along with every request, not stored there.
  hubCacheTtlMinutes: number;
}

const DEFAULTS: AppSettings = { historySize: 12, newestFirst: true, hubCacheTtlMinutes: 10 };

function load(): AppSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? { ...DEFAULTS, ...(JSON.parse(raw) as Partial<AppSettings>) } : { ...DEFAULTS };
  } catch {
    return { ...DEFAULTS };
  }
}

export const settings = $state<AppSettings>(load());

export function updateSettings(patch: Partial<AppSettings>): void {
  Object.assign(settings, patch);
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // Storage can be unavailable (private mode, quota); settings just won't survive a restart.
  }
}
