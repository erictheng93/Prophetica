import type { ReactNode } from "react";
import { ProphFooter, ProphSubNav } from "@prophetica/ui";
import {
  AuthCta,
  Brand,
  FooterLegalRow,
  legalColumn,
  MainNavLinks,
  supportColumn,
} from "@/lib/site-nav";

// SPEC-012 §2 service channels — shared chrome for /support/*.

const sectionLinks = [
  { label: "概覽", href: "/support" },
  { label: "常見問題", href: "/support/faq" },
  { label: "申訴流程", href: "/support/disputes" },
  { label: "聯絡我們", href: "/support/contact" },
];

const sectionFooterColumns = [supportColumn, legalColumn];

function SupportSectionNav() {
  return (
    <nav
      aria-label="客服 區段導覽"
      className="border-b border-proph-divider-soft bg-proph-canvas-parchment"
    >
      <div className="mx-auto max-w-proph overflow-x-auto px-6 md:px-10">
        <ul className="flex items-center gap-2 py-3">
          {sectionLinks.map((l) => (
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

export default function SupportLayout({ children }: { children: ReactNode }) {
  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav category={<Brand />} links={<MainNavLinks />} cta={<AuthCta />} />
      <SupportSectionNav />
      {children}
      <ProphFooter columns={sectionFooterColumns} legal={<FooterLegalRow />} />
    </main>
  );
}
