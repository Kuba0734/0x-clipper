import { ChevronDown } from "lucide-react";

interface Option<T extends string> {
  value: T;
  label: string;
}

interface Props<T extends string> {
  value: T;
  options: Option<T>[];
  onChange: (v: T) => void;
  label?: string;
  className?: string;
  disabled?: boolean;
}

/** Natives Select (leichtgewichtig, barrierefrei), im Design-System gestylt. */
export function Select<T extends string>({ value, options, onChange, label, className = "", disabled }: Props<T>) {
  return (
    <div className={`relative ${className}`}>
      <select
        aria-label={label}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value as T)}
        className="h-9 w-full cursor-pointer appearance-none truncate rounded-[10px] border border-border bg-surface-2 pr-9 pl-3 text-[13px] text-text transition-colors outline-none hover:border-muted/40 focus-visible:border-accent disabled:opacity-40"
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      <ChevronDown className="pointer-events-none absolute top-1/2 right-3 size-4 -translate-y-1/2 text-muted" />
    </div>
  );
}
