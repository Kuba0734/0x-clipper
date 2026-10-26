import { m } from "motion/react";

import { spring } from "@/lib/motion";

interface Props {
  checked: boolean;
  onChange: (v: boolean) => void;
  label?: string;
  disabled?: boolean;
}

/** Schalter; der Knopf bewegt sich per `layout`-Animation (FLIP = transform). */
export function Toggle({ checked, onChange, label, disabled }: Props) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative flex h-6 w-11 shrink-0 items-center rounded-full p-0.5 transition-colors duration-200 disabled:opacity-40 ${
        checked ? "justify-end bg-gradient-accent" : "justify-start bg-surface-2 ring-1 ring-border"
      }`}
    >
      <m.span layout transition={spring} className="block size-5 rounded-full bg-white shadow-sm" />
    </button>
  );
}
