import { m, type HTMLMotionProps } from "motion/react";

import { hoverLift, itemVariants, spring } from "@/lib/motion";

interface Props extends HTMLMotionProps<"div"> {
  interactive?: boolean;
}

/** Karte mit gestaffeltem Einblenden und optionalem Hover-Lift. */
export function Card({ interactive, className = "", children, ...rest }: Props) {
  return (
    <m.div
      variants={itemVariants}
      whileHover={interactive ? hoverLift : undefined}
      transition={spring}
      className={`rounded-card border border-border bg-surface ${className}`}
      {...rest}
    >
      {children}
    </m.div>
  );
}

export function Skeleton({ className = "" }: { className?: string }) {
  return <div className={`skeleton rounded-[10px] ${className}`} />;
}
