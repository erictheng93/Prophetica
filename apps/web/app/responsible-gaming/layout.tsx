import type { ReactNode } from "react";
import { ProphFooter, ProphLinkButton, ProphSubNav } from "@prophetica/ui";
import { RGHelpline } from "@/components/rg-helpline";

// Shared chrome for /responsible-gaming/* — SPEC-006 §7.3 dedicated RG section.
// Single primary nav (consolidated from prior global+sub stack) + an inline
// RG-section chip row + helpline strip + footer.

const navLinks = [
  { label: "市場", href: "/markets" },
  { label: "玩法", href: "/how-it-works" },
  { label: "責任博弈", href: "/responsible-gaming" },
  { label: "客服", href: "/support" },
];

const sectionLinks = [
  { label: "概覽", href: "/responsible-gaming" },
  { label: "限額設定", href: "/responsible-gaming/limits" },
  { label: "冷靜期", href: "/responsible-gaming/cooldown" },
  { label: "自我排除", href: "/responsible-gaming/self-exclusion" },
  { label: "求助資源", href: "/responsible-gaming/resources" },
];

const footerColumns = [
  {
    heading: "責任博弈",
    links: sectionLinks,
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

/*
 * RG section navigation — rendered as inline content (not sticky chrome) so
 * it doesn't visually stack with the primary nav as a "two-row header." Sits
 * directly above page content. Horizontally scrollable on narrow viewports;
 * each chip is a regular anchor.
 */
function RGSectionNav() {
  return (
    <nav
      aria-label="責任博弈 區段導覽"
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

export default function ResponsibleGamingLayout({ children }: { children: ReactNode }) {
  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav
        category={
          <a href="/" className="text-proph-ink hover:opacity-80">
            prophetica
          </a>
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

      <RGSectionNav />

      {children}

      <RGHelpline />

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
