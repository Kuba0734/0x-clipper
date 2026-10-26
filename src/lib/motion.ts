import type { Transition, Variants } from "motion/react";

const easeOut = [0.2, 0, 0, 1] as const;
const easeIn = [0.4, 0, 1, 1] as const;

/** Standard-Spring aus dem Design-System. */
export const spring: Transition = { type: "spring", stiffness: 380, damping: 30 };
export const fast: Transition = { duration: 0.18, ease: easeOut };

/** Seitenwechsel: Fade + 8 px Slide. */
export const pageVariants: Variants = {
  initial: { opacity: 0, y: 8 },
  animate: { opacity: 1, y: 0, transition: { duration: 0.22, ease: easeOut } },
  exit: { opacity: 0, y: -8, transition: { duration: 0.15, ease: easeIn } },
};

/** Gestaffeltes Einblenden von Karten. */
export const listVariants: Variants = {
  animate: { transition: { staggerChildren: 0.03 } },
};
export const itemVariants: Variants = {
  initial: { opacity: 0, y: 6 },
  animate: { opacity: 1, y: 0, transition: spring },
};

/** Hover-Lift für Karten. */
export const hoverLift = { y: -2, scale: 1.01 };
