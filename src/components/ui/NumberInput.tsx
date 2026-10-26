import { useState } from "react";

interface Props {
  value: number;
  onChange: (v: number) => void;
  min?: number;
  max?: number;
  suffix?: string;
  label?: string;
  width?: string;
}

/** Zahlenfeld, das erst beim Verlassen/Enter übernimmt (kein Speichern pro Tastendruck). */
export function NumberInput({ value, onChange, min, max, suffix, label, width = "w-24" }: Props) {
  // Entwurf nur während der Eingabe; sonst wird immer der aktuelle Wert angezeigt.
  const [draft, setDraft] = useState<string | null>(null);
  const text = draft ?? String(value);
  const commit = () => {
    setDraft(null);
    let v = Number(text);
    if (!Number.isFinite(v) || text === "") return;
    if (min !== undefined) v = Math.max(min, v);
    if (max !== undefined) v = Math.min(max, v);
    if (v !== value) onChange(v);
  };
  const setText = (s: string) => setDraft(s);
  return (
    <div className={`relative ${width}`}>
      <input
        aria-label={label}
        inputMode="numeric"
        value={text}
        onChange={(e) => setText(e.target.value.replace(/[^\d.]/g, ""))}
        onBlur={commit}
        onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
        className={`tabular h-9 w-full rounded-[10px] border border-border bg-surface-2 pl-3 text-[13px] text-text outline-none focus-visible:border-accent ${suffix ? "pr-12" : "pr-3"}`}
      />
      {suffix && (
        <span className="pointer-events-none absolute top-1/2 right-3 -translate-y-1/2 text-[12px] text-muted">
          {suffix}
        </span>
      )}
    </div>
  );
}
