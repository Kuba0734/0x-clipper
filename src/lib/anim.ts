// Animations-Hilfen ohne Library: Ein-/Ausblend-Zustände und hochzählende Zahlen.
import { useEffect, useRef, useState, useSyncExternalStore } from "react";

import { useSettings } from "@/stores/settings";

const mq = typeof window !== "undefined" ? window.matchMedia("(prefers-reduced-motion: reduce)") : null;

function subscribe(cb: () => void) {
  mq?.addEventListener("change", cb);
  return () => mq?.removeEventListener("change", cb);
}

/** true, wenn Windows oder die App-Einstellung reduzierte Bewegung verlangt. */
export function useReducedMotion(): boolean {
  const system = useSyncExternalStore(subscribe, () => mq?.matches ?? false);
  const own = useSettings((s) => s.settings?.general.reduceMotion ?? false);
  return system || own;
}

/** Entspricht --dur aus tokens.css (Dauer der Ausblend-Animationen). */
export const EXIT_MS = 220;

/**
 * Hält ein Element nach `open = false` noch so lange gemountet, bis seine
 * Ausblend-Animation gelaufen ist. `closing` schaltet auf die Exit-Klasse um.
 */
export function usePresence(open: boolean, exitMs = EXIT_MS) {
  const reduce = useReducedMotion();
  const [mounted, setMounted] = useState(open);
  const [closing, setClosing] = useState(false);
  const [prevOpen, setPrevOpen] = useState(open);

  // Zustandsabgleich während des Renderns (statt setState im Effect).
  if (open !== prevOpen) {
    setPrevOpen(open);
    if (open) {
      setMounted(true);
      setClosing(false);
    } else if (mounted) {
      setClosing(true);
    }
  }

  useEffect(() => {
    if (!closing) return;
    const t = window.setTimeout(() => {
      setMounted(false);
      setClosing(false);
    }, reduce ? 0 : exitMs);
    return () => window.clearTimeout(t);
  }, [closing, exitMs, reduce]);

  return { mounted, closing };
}

/** Zählt animiert zum Zielwert hoch (requestAnimationFrame, ease-out). */
export function useCountUp(target: number, ms = 450): number {
  const reduce = useReducedMotion();
  const [shown, setShown] = useState(target);
  const from = useRef(target);
  const raf = useRef(0);

  useEffect(() => {
    cancelAnimationFrame(raf.current);
    const start = performance.now();
    const origin = from.current;
    if (reduce || origin === target) {
      from.current = target;
      raf.current = requestAnimationFrame(() => setShown(target));
      return () => cancelAnimationFrame(raf.current);
    }
    const step = (now: number) => {
      const p = Math.min(1, (now - start) / ms);
      const eased = 1 - Math.pow(1 - p, 3);
      const v = origin + (target - origin) * eased;
      from.current = v;
      setShown(v);
      if (p < 1) raf.current = requestAnimationFrame(step);
    };
    raf.current = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf.current);
  }, [target, ms, reduce]);

  return shown;
}
