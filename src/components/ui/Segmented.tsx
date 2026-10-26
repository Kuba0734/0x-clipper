import { m } from "motion/react";
import { useId } from "react";

import { spring } from "@/lib/motion";

interface Props<T extends string> {
  value: T;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
  label?: string;
}

/** Segment-Umschalter; die Markierung gleitet per `layoutId` (transform). */
export function Segmented<T extends string>({ value, options, onChange, label }: Props<T>) {
  const group = useId();
  return (
    <div role="radiogroup" aria-label={label} className="inline-flex rounded-[12px] bg-surface-2 p-1 ring-1 ring-border">
      {options.map((o) => {
        const active = o.value === value;
        return (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={active}
            onClick={() => onChange(o.value)}
            className={`relative h-7 rounded-[9px] px-3 text-[13px] font-medium transition-colors ${
              active ? "text-white" : "text-muted hover:text-text"
            }`}
          >
            {active && (
              <m.span
                layoutId={`seg-${group}`}
                transition={spring}
                className="absolute inset-0 rounded-[9px] bg-gradient-accent"
              />
            )}
            <span className="relative">{o.label}</span>
          </button>
        );
      })}
    </div>
  );
}
