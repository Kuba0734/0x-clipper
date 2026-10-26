import type { Settings } from "@/lib/ipc";
import { useSettings } from "@/stores/settings";

/** Liefert die aktuellen Einstellungen (nie null innerhalb der Sektionen) und einen Setter
 *  für einen Teilbereich. Jede Änderung wird entprellt im Core gespeichert. */
export function useSection<K extends keyof Settings>(key: K) {
  const value = useSettings((s) => s.settings?.[key]) as Settings[K];
  const update = useSettings((s) => s.update);
  const set = (patch: Partial<Settings[K]>) =>
    update((s) => ({ ...s, [key]: { ...(s[key] as object), ...patch } }));
  return [value, set] as const;
}
