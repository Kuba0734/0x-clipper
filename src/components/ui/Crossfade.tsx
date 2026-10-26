import { useEffect, useState, type ReactNode } from "react";

import { EXIT_MS } from "@/lib/anim";

interface Props<K extends string> {
  /** Aktiver Inhalt (z. B. Tab-ID). */
  active: K;
  render: (k: K) => ReactNode;
  className?: string;
}

/**
 * Crossfade zwischen Tabs: Der neue Inhalt steht sofort im Fluss und blendet ein,
 * der alte liegt währenddessen absolut darüber und blendet aus (ohne Klicks anzunehmen).
 * Beide behalten ihren Key, damit der alte nicht neu gemountet wird.
 */
export function Crossfade<K extends string>({ active, render, className = "" }: Props<K>) {
  const [shown, setShown] = useState(active);
  const [leaving, setLeaving] = useState<K | null>(null);
  if (active !== shown) {
    setLeaving(shown);
    setShown(active);
  }
  useEffect(() => {
    if (!leaving) return;
    const t = window.setTimeout(() => setLeaving(null), EXIT_MS);
    return () => window.clearTimeout(t);
  }, [leaving]);

  const keys = leaving && leaving !== shown ? [leaving, shown] : [shown];
  return (
    <div className={`relative ${className}`}>
      {keys.map((k) => {
        const out = k !== shown;
        return (
          <div
            key={k}
            aria-hidden={out || undefined}
            className={out ? "anim-fade-out pointer-events-none absolute inset-x-0 top-0" : "anim-fade-in"}
          >
            {render(k)}
          </div>
        );
      })}
    </div>
  );
}
