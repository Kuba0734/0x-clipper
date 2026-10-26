import { m } from "motion/react";
import { useId } from "react";

import { spring } from "@/lib/motion";

interface Props {
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (v: number) => void;
  format?: (v: number) => string;
  label?: string;
  disabled?: boolean;
}

/**
 * Slider mit nativem <input type=range> für Tastatur/Screenreader.
 * Füllung per scaleX, Knopf per translateX – nur Transforms.
 */
export function Slider({ value, min, max, step = 1, onChange, format, label, disabled }: Props) {
  const id = useId();
  const pct = max > min ? ((value - min) / (max - min)) * 100 : 0;
  return (
    <div className={`flex w-full items-center gap-3 ${disabled ? "opacity-40" : ""}`}>
      <div className="relative h-6 flex-1">
        <div className="absolute inset-x-0 top-1/2 h-1.5 -translate-y-1/2 overflow-hidden rounded-full bg-surface-2 ring-1 ring-border">
          <m.div
            className="h-full w-full origin-left bg-gradient-accent"
            animate={{ scaleX: pct / 100 }}
            transition={spring}
          />
        </div>
        <m.div
          className="pointer-events-none absolute inset-y-0 left-0 w-full"
          animate={{ x: `${pct}%` }}
          transition={spring}
        >
          <span className="absolute top-1/2 left-0 block size-4 -translate-x-1/2 -translate-y-1/2 rounded-full bg-white shadow-[0_0_0_4px_color-mix(in_srgb,var(--accent)_30%,transparent)]" />
        </m.div>
        <input
          id={id}
          type="range"
          aria-label={label}
          min={min}
          max={max}
          step={step}
          value={value}
          disabled={disabled}
          onChange={(e) => onChange(Number(e.target.value))}
          className="absolute inset-0 h-full w-full cursor-pointer opacity-0"
        />
      </div>
      {format && (
        <label htmlFor={id} className="tabular w-20 shrink-0 text-right text-[13px] text-muted">
          {format(value)}
        </label>
      )}
    </div>
  );
}
