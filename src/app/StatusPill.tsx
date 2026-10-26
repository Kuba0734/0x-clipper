import { useT } from "@/i18n";
import { formatDuration } from "@/lib/format";
import { useStatus } from "@/stores/status";

/** Kompakter Zustand in der Titelleiste; pulsierender roter Punkt bei Aufnahme. */
export function StatusPill() {
  const t = useT();
  const status = useStatus((s) => s.status);
  if (!status) return null;
  const state = status.state;
  const dot =
    state === "recording" || state === "error" ? "bg-danger" : state === "buffering" ? "bg-accent" : "bg-subtle";
  return (
    <div className="flex h-7 items-center gap-2 rounded-full border border-border bg-surface px-3">
      <span className="relative flex size-2">
        {state === "recording" && <span className="pulse absolute inset-0 rounded-full bg-danger" />}
        <span className={`relative size-2 rounded-full ${dot}`} />
      </span>
      <span key={state} className="anim-fade-in tabular text-[12px] font-medium text-text">
        {t(`state.${state}`)}
        {status.recording && ` · ${formatDuration(status.recordingSecs)}`}
      </span>
    </div>
  );
}
