import type { ReactNode } from "react";
import { cn } from "./cn";

export interface ProphSubNavProps {
  /** Surface category in typography.tagline (21px / 600), e.g. "Markets", "Trade", "Vault". */
  category: ReactNode;
  /** Inline category nav links, hidden on small viewports. */
  links?: ReactNode;
  /** Persistent right-aligned CTA (typically a ProphButton variant="primary"). */
  cta?: ReactNode;
  className?: string;
}

/*
 * DESIGN.md §"Top Navigation" — sub-nav-frosted.
 * - background canvas-parchment at 80% opacity + backdrop-filter blur(20px) saturate(180%)
 * - height 52px, sticky directly below global-nav
 * - left: category at typography.tagline; right: inline links + persistent CTA
 *
 * Backdrop-filter is the depth signal. No drop-shadow.
 */
export function ProphSubNav({
  category,
  links,
  cta,
  className,
}: ProphSubNavProps) {
  return (
    <div
      className={cn(
        // Now the primary nav across the site (not stacked under a black
        // global-nav). Sticks to viewport top with the highest z-index.
        "sticky top-0 z-50 w-full",
        // canvas-parchment at 80% opacity — fallback to opaque if backdrop-filter unsupported
        "bg-[rgba(245,245,247,0.8)] text-proph-ink",
        "supports-[backdrop-filter]:bg-[rgba(245,245,247,0.72)]",
        "[backdrop-filter:saturate(180%)_blur(20px)]",
        "border-b border-proph-divider-soft",
        className,
      )}
      aria-label="Section"
    >
      <div className="mx-auto flex min-h-[52px] max-w-[1440px] items-center justify-between gap-3 px-5 sm:px-6 md:px-10">
        <div className="min-w-0 truncate font-display text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
          {category}
        </div>

        <div className="hidden items-center gap-6 md:flex">{links}</div>

        <div className="flex shrink-0 items-center">{cta}</div>
      </div>
      {links ? (
        <div className="border-t border-proph-divider-soft md:hidden">
          <div className="mx-auto max-w-[1440px] overflow-x-auto overscroll-x-contain px-5 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden sm:px-6">
            <div className="flex min-h-11 snap-x snap-mandatory items-center gap-5 whitespace-nowrap">
              {links}
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
