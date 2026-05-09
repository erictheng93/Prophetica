import type { ReactNode } from "react";
import { cn } from "@prophetica/ui";
import type { OpsTone } from "@/lib/ops-facade";

const toneClasses: Record<OpsTone, string> = {
  neutral: "border-proph-hairline bg-proph-surface-pearl text-proph-ink-muted-80",
  warning: "border-proph-feedback-warning bg-proph-canvas-parchment text-proph-ink",
  success: "border-proph-feedback-success bg-proph-canvas text-proph-ink",
  critical: "border-proph-feedback-error bg-proph-canvas text-proph-feedback-error",
};

interface StatusPillProps {
  children: ReactNode;
  tone?: OpsTone;
  className?: string;
}

export function StatusPill({ children, tone = "neutral", className }: StatusPillProps) {
  return (
    <span
      className={cn(
        "inline-flex min-h-7 items-center rounded-proph-pill border px-3 text-[12px] font-semibold leading-none",
        toneClasses[tone],
        className,
      )}
    >
      {children}
    </span>
  );
}
