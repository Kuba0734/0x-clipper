import { m, type HTMLMotionProps } from "motion/react";
import type { ReactNode } from "react";

import { fast } from "@/lib/motion";

type Variant = "primary" | "secondary" | "ghost" | "danger";

interface Props extends Omit<HTMLMotionProps<"button">, "children"> {
  variant?: Variant;
  size?: "sm" | "md" | "lg";
  icon?: ReactNode;
  children?: ReactNode;
}

const variants: Record<Variant, string> = {
  primary: "bg-gradient-accent text-white shadow-[0_6px_24px_-8px_var(--accent)]",
  secondary: "bg-surface-2 text-text border border-border hover:border-muted/40",
  ghost: "text-muted hover:text-text hover:bg-surface-2",
  danger: "bg-danger text-white shadow-[0_6px_24px_-10px_var(--danger)]",
};

const sizes = {
  sm: "h-8 px-3 text-[13px] gap-1.5 rounded-[10px]",
  md: "h-10 px-4 text-sm gap-2 rounded-[12px]",
  lg: "h-12 px-5 text-[15px] gap-2.5 rounded-[var(--radius)]",
};

export function Button({ variant = "secondary", size = "md", icon, children, className = "", ...rest }: Props) {
  return (
    <m.button
      type="button"
      whileHover={{ y: -1 }}
      whileTap={{ scale: 0.97 }}
      transition={fast}
      className={`inline-flex items-center justify-center font-medium transition-colors select-none disabled:opacity-40 disabled:pointer-events-none ${variants[variant]} ${sizes[size]} ${className}`}
      {...rest}
    >
      {icon}
      {children}
    </m.button>
  );
}

export function IconButton({ label, children, className = "", ...rest }: Props & { label: string }) {
  return (
    <m.button
      type="button"
      aria-label={label}
      title={label}
      whileTap={{ scale: 0.92 }}
      transition={fast}
      className={`inline-flex size-8 items-center justify-center rounded-[10px] text-muted transition-colors hover:bg-surface-2 hover:text-text ${className}`}
      {...rest}
    >
      {children}
    </m.button>
  );
}
