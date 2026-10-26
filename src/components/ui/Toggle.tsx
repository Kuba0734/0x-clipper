interface Props {
  checked: boolean;
  onChange: (v: boolean) => void;
  label?: string;
  disabled?: boolean;
}

/** Schalter: Knopf per translateX, aktive Fläche per opacity eingeblendet. */
export function Toggle({ checked, onChange, label, disabled }: Props) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className="press relative h-6 w-11 shrink-0 rounded-full border border-border-strong bg-surface-2 disabled:opacity-35"
    >
      <span
        aria-hidden
        className="fade absolute inset-[-1px] rounded-full bg-accent"
        style={{ opacity: checked ? 1 : 0 }}
      />
      <span
        aria-hidden
        className="tween absolute top-[2px] left-[2px] block size-[18px] rounded-full bg-text shadow-sm"
        style={{ transform: `translateX(${checked ? 20 : 0}px)` }}
      >
        {/* Dunkler Knopf auf weißer Fläche: per opacity statt Farbwechsel. */}
        <span className="fade absolute inset-0 rounded-full bg-on-accent" style={{ opacity: checked ? 1 : 0 }} />
      </span>
    </button>
  );
}
