import type { ButtonHTMLAttributes, ReactNode } from "react";

type Variant = "primary" | "secondary" | "ghost" | "danger";

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: "sm" | "md" | "lg";
  icon?: ReactNode;
}

const variants: Record<Variant, string> = {
  // Akzent = Weiß, Text darauf = --on-accent.
  primary: "bg-accent text-on-accent lift",
  secondary: "border border-border bg-surface text-text hoverable lift",
  ghost: "text-muted hoverable hover:text-text",
  danger: "bg-danger text-white lift",
};

const sizes = {
  sm: "h-8 px-3 text-[13px] gap-1.5 rounded-sm",
  md: "h-10 px-4 text-sm gap-2 rounded-md",
  lg: "h-12 px-5 text-[15px] gap-2.5 rounded-lg",
};

export function Button({ variant = "secondary", size = "md", icon, children, className = "", ...rest }: Props) {
  return (
    <button
      type="button"
      className={`press inline-flex items-center justify-center font-medium select-none disabled:pointer-events-none disabled:opacity-35 ${variants[variant]} ${sizes[size]} ${className}`}
      {...rest}
    >
      {icon}
      {children}
    </button>
  );
}

export function IconButton({ label, children, className = "", ...rest }: Props & { label: string }) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className={`press hoverable inline-flex size-8 items-center justify-center rounded-sm text-muted hover:text-text ${className}`}
      {...rest}
    >
      {children}
    </button>
  );
}
