import type { ReactNode } from "react";
import { ProphFooter, ProphSubNav } from "@prophetica/ui";
import {
  AuthCta,
  Brand,
  FooterLegalRow,
  kycColumn,
  legalColumn,
  MainNavLinks,
  supportColumn,
} from "@/lib/site-nav";

// SPEC-005 §8 KYC user UX — shared chrome for /kyc/*.
// Single primary nav + inline KYC section chip row + standard footer.
// (No mandatory help-line strip here — that's SPEC-006 RG-specific.)

// Chip nav reuses the footer column links and appends 審核狀態 — a transient
// state page useful to jump to from any KYC sub-page but not worth listing
// as a permanent footer category.
const sectionLinks = [...kycColumn.links, { label: "審核狀態", href: "/kyc/status" }];

const sectionFooterColumns = [kycColumn, legalColumn, supportColumn];

function KYCSectionNav() {
  return (
    <nav
      aria-label="KYC 區段導覽"
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

export default function KYCLayout({ children }: { children: ReactNode }) {
  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav category={<Brand />} links={<MainNavLinks />} cta={<AuthCta />} />

      <KYCSectionNav />

      {children}

      <ProphFooter columns={sectionFooterColumns} legal={<FooterLegalRow />} />
    </main>
  );
}
