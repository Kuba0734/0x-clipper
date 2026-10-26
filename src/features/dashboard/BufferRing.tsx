import type { CSSProperties, ReactNode } from "react";

interface Props {
  /** 0–1 */
  fill: number;
  recording: boolean;
  active: boolean;
  children?: ReactNode;
}

const SIZE = 248;
const STROKE = 12;

/**
 * Füllstand-Ring ausschließlich über transform: zwei Halbringe drehen sich in je einer
 * geclippten Hälfte (rechts 0–50 %, links 50–100 %). Der Wert kommt 2×/s vom Core,
 * die Transition glättet dazwischen.
 */
export function BufferRing({ fill, recording, active, children }: Props) {
  const p = Math.max(0, Math.min(1, fill));
  const right = Math.min(p, 0.5) * 360;
  const left = Math.max(0, p - 0.5) * 360;
  const color = recording ? "var(--danger)" : "var(--accent)";
  const ring: CSSProperties = {
    position: "absolute",
    inset: 0,
    borderRadius: "50%",
    border: `${STROKE}px solid ${color}`,
  };
  return (
    <div className="relative grid place-items-center" style={{ width: SIZE, height: SIZE }}>
      {/* Weicher Lichthof – nur opacity ändert sich. */}
      <div
        aria-hidden
        className="fade absolute inset-8 rounded-full blur-3xl"
        style={{ background: color, opacity: active ? (recording ? 0.22 : 0.08) : 0 }}
      />
      {/* Spur */}
      <div aria-hidden className="absolute inset-0 rounded-full" style={{ border: `${STROKE}px solid var(--surface-2)` }} />
      {/* Rechte Hälfte: zeigt 0–180°. Der linke Halbring dreht sich hinein. */}
      <div aria-hidden className="absolute inset-0" style={{ clipPath: "inset(0 0 0 50%)" }}>
        <div className="tween absolute inset-0" style={{ transform: `rotate(${right}deg)` }}>
          <div style={{ ...ring, clipPath: "inset(0 50% 0 0)" }} />
        </div>
      </div>
      {/* Linke Hälfte: zeigt 180–360°. Der rechte Halbring dreht sich hinein. */}
      <div aria-hidden className="absolute inset-0" style={{ clipPath: "inset(0 50% 0 0)" }}>
        <div className="tween absolute inset-0" style={{ transform: `rotate(${left}deg)` }}>
          <div style={{ ...ring, clipPath: "inset(0 0 0 50%)" }} />
        </div>
      </div>
      <div className="absolute inset-0 grid place-items-center text-center">{children}</div>
    </div>
  );
}
