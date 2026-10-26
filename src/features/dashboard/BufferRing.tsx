import { m, useSpring, useTransform } from "motion/react";
import { useEffect, useId } from "react";

interface Props {
  /** 0–1 */
  fill: number;
  recording: boolean;
  active: boolean;
  children?: React.ReactNode;
}

const SIZE = 248;
const STROKE = 14;
const R = (SIZE - STROKE) / 2;

/**
 * Großer Füllstand-Ring. Der Wert kommt 2×/s vom Core und wird per Spring geglättet;
 * gezeichnet über pathLength (wie das Häkchen im Toast).
 */
export function BufferRing({ fill, recording, active, children }: Props) {
  const id = useId();
  const v = useSpring(0, { stiffness: 120, damping: 24 });
  useEffect(() => v.set(Math.max(0, Math.min(1, fill))), [fill, v]);
  const opacity = useTransform(v, [0, 0.02], [0, 1]);

  return (
    <div className="relative grid place-items-center" style={{ width: SIZE, height: SIZE }}>
      {/* Weicher Glow hinter dem Ring – nur opacity animiert. */}
      <m.div
        aria-hidden
        className="absolute inset-6 rounded-full blur-3xl"
        style={{ background: recording ? "var(--danger)" : "var(--gradient)" }}
        animate={{ opacity: active ? (recording ? [0.25, 0.45, 0.25] : 0.22) : 0.06 }}
        transition={recording ? { duration: 1.6, repeat: Infinity } : { duration: 0.4 }}
      />
      <svg width={SIZE} height={SIZE} viewBox={`0 0 ${SIZE} ${SIZE}`} className="relative -rotate-90">
        <defs>
          <linearGradient id={`g-${id}`} x1="0" y1="0" x2="1" y2="1">
            <stop offset="0%" stopColor="var(--accent)" />
            <stop offset="100%" stopColor="var(--accent-2)" />
          </linearGradient>
        </defs>
        <circle cx={SIZE / 2} cy={SIZE / 2} r={R} fill="none" stroke="var(--surface-2)" strokeWidth={STROKE} />
        <m.circle
          cx={SIZE / 2}
          cy={SIZE / 2}
          r={R}
          fill="none"
          stroke={recording ? "var(--danger)" : `url(#g-${id})`}
          strokeWidth={STROKE}
          strokeLinecap="round"
          style={{ pathLength: v, opacity }}
        />
      </svg>
      <div className="absolute inset-0 grid place-items-center text-center">{children}</div>
    </div>
  );
}
