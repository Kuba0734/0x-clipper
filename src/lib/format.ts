export function formatBytes(b: number): string {
  if (b < 1024) return `${b} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = b / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v >= 100 ? 0 : 1)} ${units[i]}`;
}

export function formatDuration(secs: number): string {
  const t = Math.max(0, Math.round(secs));
  const h = Math.floor(t / 3600);
  const m = Math.floor((t % 3600) / 60);
  const s = t % 60;
  const mm = h > 0 ? String(m).padStart(2, "0") : String(m);
  return `${h > 0 ? `${h}:` : ""}${mm}:${String(s).padStart(2, "0")}`;
}

export function formatDate(ms: number, lang: "de" | "en"): string {
  return new Intl.DateTimeFormat(lang === "de" ? "de-DE" : "en-US", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(ms));
}

/** Schöner Anzeigename eines Hotkeys: „Alt+F10“ → ["Alt", "F10"]. */
export function keycaps(accel: string): string[] {
  return accel
    .split("+")
    .map((k) => k.trim())
    .filter(Boolean)
    .map((k) => (k === "Control" ? "Ctrl" : k));
}
