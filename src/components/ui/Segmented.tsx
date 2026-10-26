interface Props<T extends string> {
  value: T;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
  label?: string;
}

/**
 * Segment-Umschalter mit gleich breiten Segmenten; die weiße Markierung gleitet
 * per translateX (Index × 100 %) – keine Breiten- oder Positionsanimation.
 */
export function Segmented<T extends string>({ value, options, onChange, label }: Props<T>) {
  const idx = Math.max(0, options.findIndex((o) => o.value === value));
  return (
    <div
      role="radiogroup"
      aria-label={label}
      className="relative grid rounded-md border border-border bg-surface p-1"
      style={{ gridTemplateColumns: `repeat(${options.length}, minmax(0, 1fr))` }}
    >
      <span
        aria-hidden
        className="slide-indicator absolute top-1 bottom-1 left-1 rounded-sm bg-accent"
        style={{
          width: `calc((100% - 8px) / ${options.length})`,
          transform: `translateX(${idx * 100}%)`,
        }}
      />
      {options.map((o) => {
        const active = o.value === value;
        return (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={active}
            onClick={() => onChange(o.value)}
            className={`press relative h-7 truncate rounded-sm px-3 text-[13px] font-medium ${
              active ? "text-on-accent" : "text-muted hover:text-text"
            }`}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}
