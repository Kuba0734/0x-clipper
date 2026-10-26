import type { CSSProperties, HTMLAttributes } from "react";

interface Props extends HTMLAttributes<HTMLDivElement> {
  interactive?: boolean;
  /** Position in einer gestaffelt einblendenden Liste (max. ~10 wirken). */
  index?: number;
}

/** Glas-Panel ohne Blur; optional mit Hover-Lift und gestaffeltem Einblenden. */
export function Card({ interactive, index, className = "", style, children, ...rest }: Props) {
  const stagger = index !== undefined ? "stagger" : "";
  return (
    <div
      className={`panel ${interactive ? "lift press cursor-pointer" : ""} ${stagger} ${className}`}
      style={index !== undefined ? ({ ...style, "--i": index } as CSSProperties) : style}
      {...rest}
    >
      {children}
    </div>
  );
}

export function Skeleton({ className = "" }: { className?: string }) {
  return <div className={`skeleton rounded-md ${className}`} />;
}
