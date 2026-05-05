import type { ReactNode } from "react";
import Link from "next/link";
import { ProphFooter, ProphLinkButton, ProphSubNav } from "@prophetica/ui";

// SPEC-005 §8 KYC user UX — shared chrome for /kyc/*.
// Single primary nav + inline KYC section chip row + standard footer.
// (No mandatory help-line strip here — that's SPEC-006 RG-specific.)

const navLinks = [
  { label: "市場", href: "/markets" },
  { label: "玩法", href: "/how-it-works" },
  { label: "責任博弈", href: "/responsible-gaming" },
  { label: "客服", href: "/support" },
];

const sectionLinks = [
  { label: "概覽", href: "/kyc" },
  { label: "Tier 1 基礎", href: "/kyc/tier-1" },
  { label: "Tier 2 標準", href: "/kyc/tier-2" },
  { label: "Tier 3 強化", href: "/kyc/tier-3" },
  { label: "隱私說明", href: "/kyc/privacy-notice" },
  { label: "審核狀態", href: "/kyc/status" },
];

const footerColumns = [
  {
    heading: "身份驗證",
    links: [
      { label: "KYC 概覽", href: "/kyc" },
      { label: "Tier 1 基礎", href: "/kyc/tier-1" },
      { label: "Tier 2 標準", href: "/kyc/tier-2" },
      { label: "Tier 3 強化", href: "/kyc/tier-3" },
      { label: "隱私說明", href: "/kyc/privacy-notice" },
    ],
  },
  {
    heading: "法律與合規",
    links: [
      { label: "服務條款", href: "/legal/terms" },
      { label: "隱私政策", href: "/legal/privacy" },
      { label: "AML 政策", href: "/legal/aml" },
      { label: "禁止地區", href: "/legal/geo-block" },
    ],
  },
  {
    heading: "支援",
    links: [
      { label: "客服中心", href: "/support" },
      { label: "申訴流程", href: "/support/disputes" },
      { label: "聯絡我們", href: "/support/contact" },
    ],
  },
];

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
      <ProphSubNav
        category={
          <Link href="/" className="text-proph-ink hover:opacity-80">
            prophetica
          </Link>
        }
        links={navLinks.map((l) => (
          <a
            key={l.href}
            href={l.href}
            className="font-text text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-ink/70 transition-colors hover:text-proph-ink"
          >
            {l.label}
          </a>
        ))}
        cta={
          <div className="flex items-center gap-4">
            <a
              href="/auth/sign-in"
              className="font-text text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-ink/70 transition-colors hover:text-proph-ink"
            >
              登入
            </a>
            <ProphLinkButton variant="primary" href="/auth/sign-in">
              開始體驗
            </ProphLinkButton>
          </div>
        }
      />

      <KYCSectionNav />

      {children}

      <ProphFooter
        columns={footerColumns}
        legal={
          <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
            <span>
              © {new Date().getFullYear()} prophetica. 趣味預測娛樂遊戲服務 · 18+ ·
              站內點數不可兌回現金 / 虛擬貨幣 / 實體商品。
            </span>
            <span className="font-text text-[12px] leading-none tracking-[-0.12px]">
              援助專線 <a href="tel:1925" className="text-proph-primary hover:underline">1925</a>（24 小時）·{" "}
              <a href="/legal/geo-block" className="hover:underline">服務地區限制</a>
            </span>
          </div>
        }
      />
    </main>
  );
}
