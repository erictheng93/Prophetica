import type { ReactNode } from "react";
import { ProphFooter, ProphSubNav } from "@prophetica/ui";
import { RGHelpline } from "@/components/rg-helpline";
import {
  AuthCta,
  Brand,
  FooterLegalRow,
  getFooterColumn,
  getFooterColumns,
  MainNavLinks,
  type SiteNavLink,
} from "@/lib/site-nav";

// Shared chrome for /responsible-gaming/* — SPEC-006 §7.3 dedicated RG section.
// Single primary nav (consolidated from prior global+sub stack) + an inline
// RG-section chip row + helpline strip + footer.

/*
 * RG section navigation — rendered as inline content (not sticky chrome) so
 * it doesn't visually stack with the primary nav as a "two-row header." Sits
 * directly above page content. Horizontally scrollable on narrow viewports;
 * each chip is a regular anchor.
 */
function RGSectionNav({ links }: { links: SiteNavLink[] }) {
  return (
    <nav
      aria-label="責任博弈 區段導覽"
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

export default async function ResponsibleGamingLayout({ children }: { children: ReactNode }) {
  const [rgColumn, footerColumns] = await Promise.all([
    getFooterColumn("responsibleGaming"),
    getFooterColumns(["responsibleGaming", "legal", "support"]),
  ]);

  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav category={<Brand />} links={<MainNavLinks />} cta={<AuthCta />} />

      <RGSectionNav links={rgColumn.links} />

      {children}

      <RGHelpline />

      <ProphFooter columns={footerColumns} legal={<FooterLegalRow />} />
    </main>
  );
}
