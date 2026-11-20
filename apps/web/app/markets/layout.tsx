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

import { MarketsSectionNav } from "@/components/markets-section-nav";

export default async function MarketsLayout({
  children,
}: {
  children: ReactNode;
}) {
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
      <ProphSubNav
        category={<Brand />}
        links={<MainNavLinks />}
        cta={<AuthCta />}
      />
      <MarketsSectionNav links={sectionLinks} ariaLabel={tNav("ariaLabel")} />
      {children}
      <ProphFooter columns={footerColumns} legal={<FooterLegalRow />} />
    </main>
  );
}
