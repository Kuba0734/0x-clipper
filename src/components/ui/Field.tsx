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
    <div className={`flex gap-4 py-4 ${stacked ? "flex-col" : "items-center justify-between"}`}>
      <div className="min-w-0">
        <div className="text-[14px] font-medium text-text">{label}</div>
        {hint && <div className="mt-1 text-[12.5px] leading-snug text-muted">{hint}</div>}
      </div>
      <div className={stacked ? "w-full" : "flex shrink-0 items-center gap-2"}>{children}</div>
    </div>
  );
}

export function Section({ title, children, index = 0 }: { title?: string; children: ReactNode; index?: number }) {
  return (
    <section className="panel stagger px-6 py-1" style={{ "--i": index } as React.CSSProperties}>
      {title && <h3 className="pt-5 pb-1 text-[11.5px] font-semibold tracking-[0.08em] text-subtle uppercase">{title}</h3>}
      <div className="divide-y divide-border">{children}</div>
    </section>
  );
}
