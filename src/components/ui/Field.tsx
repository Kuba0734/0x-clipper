import type { ReactNode } from "react";

interface Props {
  label: string;
  hint?: ReactNode;
  children: ReactNode;
  /** Steuerelement unter statt neben dem Label (für breite Controls). */
  stacked?: boolean;
}

/** Einstellungszeile: Label + Hinweis links, Control rechts. */
export function Field({ label, hint, children, stacked }: Props) {
  return (
    <div className={`flex gap-4 py-3.5 ${stacked ? "flex-col" : "items-center justify-between"}`}>
      <div className="min-w-0">
        <div className="text-[14px] font-medium text-text">{label}</div>
        {hint && <div className="mt-0.5 text-[12.5px] leading-snug text-muted">{hint}</div>}
      </div>
      <div className={stacked ? "w-full" : "flex shrink-0 items-center gap-2"}>{children}</div>
    </div>
  );
}

export function Section({ title, children }: { title?: string; children: ReactNode }) {
  return (
    <section className="rounded-card border border-border bg-surface px-5 py-1">
      {title && (
        <h3 className="pt-4 pb-1 text-[12px] font-semibold tracking-wide text-muted uppercase">{title}</h3>
      )}
      <div className="divide-y divide-border/70">{children}</div>
    </section>
  );
}
