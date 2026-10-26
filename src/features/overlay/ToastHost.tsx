import { Icon } from "@/components/ui/Icon";
import { useToasts, type Toast } from "@/stores/toasts";

function ToastView({ toast }: { toast: Toast }) {
  const dismiss = useToasts((s) => s.dismiss);
  const icon =
    toast.kind === "success" ? (
      <span className="grid size-8 place-items-center rounded-full bg-accent text-on-accent">
        <Icon name="check" size={16} strokeWidth={2.5} />
      </span>
    ) : toast.kind === "error" ? (
      <span className="grid size-8 place-items-center rounded-full bg-danger text-white">
        <Icon name="alert" size={16} />
      </span>
    ) : (
      <span className="grid size-8 place-items-center rounded-full bg-surface-2 text-text">
        <Icon name="info" size={16} />
      </span>
    );
  return (
    <div
      role="status"
      onClick={() => dismiss(toast.id)}
      className={`glass pointer-events-auto relative w-80 cursor-pointer overflow-hidden rounded-lg shadow-[var(--shadow-modal)] ${
        toast.leaving ? "anim-toast-out" : "anim-toast-in"
      }`}
    >
      <div className="flex items-center gap-3 p-3">
        {icon}
        <div className="min-w-0">
          <div className="truncate text-[13.5px] font-semibold">{toast.title}</div>
          {toast.detail && <div className="truncate text-[12px] text-muted">{toast.detail}</div>}
        </div>
      </div>
      {/* Ablaufende Fortschrittsleiste (scaleX, linear über die Anzeigedauer). */}
      <span
        aria-hidden
        className={`absolute inset-x-0 bottom-0 h-[2px] origin-left ${toast.kind === "error" ? "bg-danger" : "bg-accent"}`}
        style={{ animation: `progress-run ${toast.durationMs}ms linear both` }}
      />
    </div>
  );
}

/** Toasts im offenen Hauptfenster. Über dem Spiel zeigt der Core ein natives Overlay. */
export function ToastHost() {
  const toasts = useToasts((s) => s.toasts);
  return (
    <div className="pointer-events-none fixed right-4 bottom-4 z-50 flex flex-col items-end gap-2">
      {toasts.map((t) => (
        <ToastView key={t.id} toast={t} />
      ))}
    </div>
  );
}
