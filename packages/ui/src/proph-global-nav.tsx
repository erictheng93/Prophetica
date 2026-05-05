import type { ReactNode } from "react";
import { cn } from "./cn";

export interface ProphGlobalNavLink {
  label: string;
  href: string;
}

export interface ProphGlobalNavProps {
  /** Brand mark rendered at the left of the bar (logo word or SVG). */
  brand: ReactNode;
  /** Primary nav items. Rendered horizontally on ≥834px, collapsed on smaller widths. */
  links: ProphGlobalNavLink[];
  /** Right-aligned utility cluster (e.g. Search icon, Sign-In, Bag). */
  utility?: ReactNode;
  className?: string;
}

/*
 * DESIGN.md §"Top Navigation" — global-nav.
 * - background surface-black (true black, the only place pure black appears)
 * - height 44px
 * - typography.nav-link (12px / 400 / -0.12px tracking)
 * - links spaced ~20px apart
 *
 * Mobile collapse strategy (DESIGN.md §"Collapsing Strategy"):
 *   ≤834px → links hidden, brand + hamburger + utility only.
 *   This component renders the desktop layout; a hamburger replacement is
 *   wired in apps/web at the layout level (TODO: add mobile drawer later).
 */
export function ProphGlobalNav({ brand, links, utility, className }: ProphGlobalNavProps) {
  return (
    <nav
      className={cn(
        "sticky top-0 z-50 w-full",
        "bg-proph-surface-black text-proph-on-dark",
        "h-11",
        className,
      )}
      aria-label="Primary"
    >
      <div className="mx-auto flex h-full max-w-[1440px] items-center justify-between px-6 md:px-10">
        <div className="flex items-center">{brand}</div>

        <ul className="hidden items-center gap-5 md:flex">
          {links.map((link) => (
            <li key={link.href}>
              <a
                href={link.href}
                className={cn(
                  "font-text text-[12px] leading-none tracking-[-0.12px]",
                  "text-proph-on-dark/80 transition-colors hover:text-proph-on-dark",
                  "focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-4",
                  "focus-visible:outline-proph-primary-focus rounded-sm",
                )}
              >
                {link.label}
              </a>
            </li>
          ))}
        </ul>

        <div className="flex items-center gap-3">{utility}</div>
      </div>
    </nav>
  );
}
