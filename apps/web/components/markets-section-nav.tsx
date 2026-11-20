"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import type { SiteNavLink } from "@/lib/site-nav";

export function MarketsSectionNav({
  links,
  ariaLabel,
}: {
  links: SiteNavLink[];
  ariaLabel: string;
}) {
  const pathname = usePathname();

  return (
    <nav
      aria-label={ariaLabel}
      className="sticky top-11 z-20 border-b border-proph-hairline/80 bg-proph-canvas/80 backdrop-blur-xl transition-all"
    >
      <div className="mx-auto max-w-proph overflow-x-auto overscroll-x-contain px-6 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden md:px-10">
        <ul className="flex snap-x snap-mandatory items-center gap-2 py-3">
          {links.map((l) => {
            const isActive =
              l.href === "/markets"
                ? pathname === "/markets"
                : pathname.startsWith(l.href);

            return (
              <li key={l.href} className="snap-start">
                <Link
                  href={l.href}
                  className={
                    "inline-flex min-h-9 items-center rounded-proph-pill px-4 py-1.5 font-text text-[13px] font-medium transition-all " +
                    (isActive
                      ? "bg-proph-ink text-proph-canvas shadow-sm dark:bg-white dark:text-black"
                      : "border border-proph-hairline bg-proph-canvas text-proph-ink-muted-80 hover:border-proph-primary/40 hover:text-proph-ink")
                  }
                >
                  {l.label}
                </Link>
              </li>
            );
          })}
        </ul>
      </div>
    </nav>
  );
}
