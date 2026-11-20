import { type HTMLAttributes } from "react";
import { cn } from "./cn";

export interface ShimmerSkeletonProps extends HTMLAttributes<HTMLDivElement> {
  className?: string;
}

export function ShimmerSkeleton({ className, ...props }: ShimmerSkeletonProps) {
  return (
    <div
      className={cn(
        "relative overflow-hidden rounded-proph-md bg-zinc-200/70 dark:bg-zinc-800/60",
        "after:absolute after:inset-0 after:-translate-x-full after:animate-[proph-shimmer_1.8s_infinite]",
        "after:bg-gradient-to-r after:from-transparent after:via-white/20 dark:after:via-white/10 after:to-transparent",
        className,
      )}
      {...props}
    />
  );
}
