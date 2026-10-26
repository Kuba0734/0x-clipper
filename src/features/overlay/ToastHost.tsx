import { AnimatePresence, m } from "motion/react";
import { AlertTriangle, Info } from "lucide-react";

import { spring } from "@/lib/motion";
import { useToasts, type Toast } from "@/stores/toasts";

/** Häkchen, das als SVG-Pfad gezeichnet wird (pathLength). */
export function DrawnCheck({ className = "" }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} fill="none" aria-hidden>
      <m.path
        d="M5 12.5l4.5 4.5L19 7.5"
        stroke="currentColor"
        strokeWidth={2.6}
        strokeLinecap="round"
        strokeLinejoin="round"
        initial={{ pathLength: 0 }}
        animate={{ pathLength: 1 }}
        transition={{ duration: 0.35, delay: 0.12, ease: "easeOut" }}
      />
    </svg>
  );
}

function ToastView({ toast }: { toast: Toast }) {
  const dismiss = useToasts((s) => s.dismiss);
  const icon =
    toast.kind === "success" ? (
      <span className="flex size-8 items-center justify-center rounded-full bg-gradient-accent text-white">
        <DrawnCheck className="size-5" />
      </span>
    ) : toast.kind === "error" ? (
      <span className="flex size-8 items-center justify-center rounded-full bg-danger text-white">
        <AlertTriangle className="size-4" />
      </span>
    ) : (
      <span className="flex size-8 items-center justify-center rounded-full bg-surface-2 text-text">
        <Info className="size-4" />
      </span>
    );
  return (
    <m.div
      layout
      initial={{ opacity: 0, x: 40, scale: 0.96 }}
      animate={{ opacity: 1, x: 0, scale: 1 }}
      exit={{ opacity: 0, x: 24, transition: { duration: 0.18 } }}
      transition={spring}
      onClick={() => dismiss(toast.id)}
      role="status"
      className="glass pointer-events-auto flex w-80 cursor-pointer items-center gap-3 rounded-card border border-border p-3 shadow-2xl"
    >
      {icon}
      <div className="min-w-0">
        <div className="truncate text-[13.5px] font-semibold">{toast.title}</div>
        {toast.detail && <div className="truncate text-[12px] text-muted">{toast.detail}</div>}
      </div>
    </m.div>
  );
}

/** Toasts im offenen Hauptfenster. Über dem Spiel zeigt der Core ein natives Overlay. */
export function ToastHost() {
  const toasts = useToasts((s) => s.toasts);
  return (
    <div className="pointer-events-none fixed right-4 bottom-4 z-50 flex flex-col items-end gap-2">
      <AnimatePresence initial={false}>
        {toasts.map((t) => (
          <ToastView key={t.id} toast={t} />
        ))}
      </AnimatePresence>
    </div>
  );
}
