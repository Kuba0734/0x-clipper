import { AnimatePresence, m } from "motion/react";

import { useT } from "@/i18n";
import { formatDuration } from "@/lib/format";
import { spring } from "@/lib/motion";
import { useStatus } from "@/stores/status";

/** Kompakter Zustand in der Titelleiste: pulsierender roter Punkt bei Aufnahme. */
export function StatusPill() {
  const t = useT();
  const status = useStatus((s) => s.status);
  if (!status) return null;
  const state = status.state;
  const color =
    state === "recording" ? "bg-danger" : state === "buffering" ? "bg-success" : state === "error" ? "bg-danger" : "bg-muted";
  return (
    <m.div layout transition={spring} className="flex h-7 items-center gap-2 rounded-full bg-surface px-3 ring-1 ring-border">
      <span className="relative flex size-2">
        {state === "recording" && (
          <m.span
            className="absolute inset-0 rounded-full bg-danger"
            animate={{ scale: [1, 2.2], opacity: [0.6, 0] }}
            transition={{ duration: 1.2, repeat: Infinity, ease: "easeOut" }}
          />
        )}
        <span className={`relative size-2 rounded-full ${color}`} />
      </span>
      <AnimatePresence mode="popLayout" initial={false}>
        <m.span
          key={state}
          initial={{ opacity: 0, y: 4 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -4 }}
          className="tabular text-[12px] font-medium text-text"
        >
          {t(`state.${state}`)}
          {status.recording && ` · ${formatDuration(status.recordingSecs)}`}
        </m.span>
      </AnimatePresence>
    </m.div>
  );
}
