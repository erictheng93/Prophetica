import type { ReactNode } from "react";
import { getTranslations } from "next-intl/server";
import { ProphFooter, ProphSubNav } from "@prophetica/ui";
import {
  AuthCta,
  Brand,
  FooterLegalRow,
  getFooterColumns,
  MainNavLinks,
  type SiteNavLink,
} from "@/lib/site-nav";

const sectionItems = [
  { key: "all", href: "/markets" },
  { key: "trending", href: "/markets/trending" },
  { key: "new", href: "/markets/new" },
  { key: "settled", href: "/markets/settled" },
];

function MarketsSectionNav({
  links,
  ariaLabel,
}: {
  links: SiteNavLink[];
  ariaLabel: string;
}) {
  return (
    <nav
      aria-label={ariaLabel}
      className="border-b border-proph-divider-soft bg-proph-canvas-parchment"
    >
      <div className="mx-auto max-w-proph overflow-x-auto overscroll-x-contain px-6 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden md:px-10">
        <ul className="flex snap-x snap-mandatory items-center gap-2 py-3">
          {links.map((l) => (
            <li key={l.href} className="snap-start">
              <a
                href={l.href}
                className="inline-flex min-h-11 items-center rounded-proph-pill border border-proph-hairline bg-proph-canvas px-4 py-2 font-text text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-ink-muted-80 transition-colors hover:border-proph-primary hover:text-proph-ink active:border-proph-primary active:text-proph-ink"
              >
                {l.label}
              </a>
            </li>
          ))}
        </ul>
      </div>
    </nav>
  );
}

export default async function MarketsLayout({ children }: { children: ReactNode }) {
  const [tNav, footerColumns] = await Promise.all([
    getTranslations("sectionNav.markets"),
    getFooterColumns(["markets", "legal", "support"]),
  ]);
  const sectionLinks: SiteNavLink[] = sectionItems.map(({ key, href }) => ({
    label: tNav(key),
    href,
  }));

  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav category={<Brand />} links={<MainNavLinks />} cta={<AuthCta />} />
      <MarketsSectionNav links={sectionLinks} ariaLabel={tNav("ariaLabel")} />
      {children}
      <ProphFooter columns={footerColumns} legal={<FooterLegalRow />} />
    </main>
  );
}
