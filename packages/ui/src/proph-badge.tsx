import { type HTMLAttributes, type ReactNode } from "react";
import { cn } from "./cn";

export type ProphBadgeTone =
  | "neutral"
  | "brand"
  | "success"
  | "warning"
  | "error"
  | "gold";

export interface ProphBadgeProps extends HTMLAttributes<HTMLSpanElement> {
  tone?: ProphBadgeTone;
  children?: ReactNode;
  pulse?: boolean;
  size?: "sm" | "md";
  className?: string;
}

const toneStyles: Record<
  ProphBadgeTone,
  { container: string; pip: string; ping: string }
> = {
  neutral: {
    container:
      "bg-proph-canvas-parchment text-proph-ink-muted-80 border-proph-divider-soft",
    pip: "bg-zinc-400",
    ping: "bg-zinc-400/50",
  },
  brand: {
    container:
      "bg-blue-500/10 text-proph-primary border-proph-primary/25 dark:text-blue-400",
    pip: "bg-blue-500 dark:bg-blue-400",
    ping: "bg-blue-500/50 dark:bg-blue-400/50",
  },
  success: {
    container:
      "bg-emerald-500/10 text-emerald-700 border-emerald-500/25 dark:text-emerald-400",
    pip: "bg-emerald-500 dark:bg-emerald-400",
    ping: "bg-emerald-500/50 dark:bg-emerald-400/50",
  },
  warning: {
    container:
      "bg-amber-500/10 text-amber-700 border-amber-500/25 dark:text-amber-300",
    pip: "bg-amber-500",
    ping: "bg-amber-500/50",
  },
  error: {
    container:
      "bg-rose-500/10 text-rose-700 border-rose-500/25 dark:text-rose-400",
    pip: "bg-rose-500 dark:bg-rose-400",
    ping: "bg-rose-500/50 dark:bg-rose-400/50",
  },
  gold: {
    container:
      "bg-[#D4A857]/15 text-[#D4A857] border-[#D4A857]/40 dark:text-[#F0D78C]",
    pip: "bg-[#F0D78C]",
    ping: "bg-[#F0D78C]/60",
  },
};

export function ProphBadge({
  tone = "neutral",
  children,
  pulse = false,
  size = "md",
  className,
  ...props
}: ProphBadgeProps) {
  const currentTone = toneStyles[tone];

  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 rounded-proph-pill border font-text font-medium leading-none select-none tracking-[-0.12px]",
        size === "sm" ? "px-2 py-0.5 text-[11px]" : "px-2.5 py-1 text-[12px]",
        currentTone.container,
        className,
      )}
      {...props}
    >
      {pulse ? (
        <span className="relative flex h-2 w-2 items-center justify-center">
          <span
            className={cn(
              "absolute inline-flex h-full w-full animate-ping rounded-full opacity-75",
              currentTone.ping,
            )}
          />
          <span
            className={cn(
              "relative inline-flex h-1.5 w-1.5 rounded-full",
              currentTone.pip,
            )}
          />
        </span>
      ) : null}
      {children}
    </span>
  );
}
