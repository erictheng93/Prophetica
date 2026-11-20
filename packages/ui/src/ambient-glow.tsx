import { type HTMLAttributes } from "react";
import { cn } from "./cn";

export type AmbientGlowVariant =
  | "brand"
  | "gold"
  | "emerald"
  | "aurora"
  | "subtle";

export interface AmbientGlowProps extends HTMLAttributes<HTMLDivElement> {
  variant?: AmbientGlowVariant;
  size?: "sm" | "md" | "lg" | "full";
  blur?: "sm" | "md" | "lg" | "xl";
  className?: string;
}

const variantStyles: Record<AmbientGlowVariant, string> = {
  brand:
    "from-blue-500/20 via-sky-400/10 to-transparent dark:from-blue-500/25 dark:via-sky-400/15",
  gold: "from-[#F0D78C]/25 via-[#D4A857]/15 to-transparent dark:from-[#F0D78C]/30 dark:via-[#D4A857]/20",
  emerald:
    "from-emerald-500/20 via-teal-400/10 to-transparent dark:from-emerald-500/25 dark:via-teal-400/15",
  aurora:
    "from-teal-500/20 via-emerald-400/15 to-amber-300/10 dark:from-teal-500/25 dark:via-emerald-500/20",
  subtle:
    "from-zinc-400/10 via-zinc-300/5 to-transparent dark:from-zinc-600/15 dark:via-zinc-700/5",
};

const sizeStyles = {
  sm: "h-48 w-48",
  md: "h-80 w-80",
  lg: "h-[480px] w-[480px]",
  full: "inset-0 h-full w-full",
};

const blurStyles = {
  sm: "blur-xl",
  md: "blur-2xl",
  lg: "blur-3xl",
  xl: "blur-[100px]",
};

export function AmbientGlow({
  variant = "brand",
  size = "md",
  blur = "lg",
  className,
  ...props
}: AmbientGlowProps) {
  return (
    <div
      aria-hidden="true"
      className={cn(
        "pointer-events-none absolute -z-10 select-none overflow-hidden rounded-full bg-gradient-to-tr opacity-70 transition-opacity duration-700",
        variantStyles[variant],
        sizeStyles[size],
        blurStyles[blur],
        className,
      )}
      {...props}
    />
  );
}
