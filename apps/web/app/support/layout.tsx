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

// SPEC-012 §2 service channels — shared chrome for /support/*.

const sectionItems = [
  { key: "overview", href: "/support" },
  { key: "faq", href: "/support/faq" },
  { key: "disputes", href: "/support/disputes" },
  { key: "contact", href: "/support/contact" },
];

function SupportSectionNav({
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
      <div className="mx-auto max-w-proph overflow-x-auto px-6 md:px-10">
        <ul className="flex items-center gap-2 py-3">
          {links.map((l) => (
            <li key={l.href}>
              <a
                href={l.href}
                className="inline-flex items-center rounded-proph-pill border border-proph-hairline bg-proph-canvas px-4 py-1.5 font-text text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-ink-muted-80 transition-colors hover:border-proph-primary hover:text-proph-ink"
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

export default async function SupportLayout({
  children,
}: {
  children: ReactNode;
}) {
  const [tNav, footerColumns] = await Promise.all([
    getTranslations("sectionNav.support"),
    getFooterColumns(["support", "legal"]),
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
      <SupportSectionNav links={sectionLinks} ariaLabel={tNav("ariaLabel")} />
      {children}
      <ProphFooter columns={footerColumns} legal={<FooterLegalRow />} />
    </main>
  );
}
