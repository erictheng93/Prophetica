import type { ReactNode } from "react";
import Link from "next/link";
import { ProphFooter, ProphLinkButton, ProphSubNav, ProphTile } from "@prophetica/ui";

// Reusable placeholder for routes that exist in the IA but lack real content.
// Keeps footer/nav links navigable (no 404) without fabricating product copy
// the SPEC hasn't written yet. Replace with the real page module when content
// is approved.

const navLinks = [
  { label: "市場", href: "/markets" },
  { label: "玩法", href: "/how-it-works" },
  { label: "責任博弈", href: "/responsible-gaming" },
  { label: "客服", href: "/support" },
];

const footerColumns = [
  {
    heading: "認識 prophetica",
    links: [
      { label: "關於平台", href: "/about" },
      { label: "撮合機制", href: "/how-it-works" },
      { label: "結算與爭議", href: "/settlement" },
      { label: "代理計畫", href: "/affiliate" },
    ],
  },
  {
    heading: "玩家中心",
    links: [
      { label: "如何儲值點數", href: "/topup" },
      { label: "身份驗證 KYC", href: "/kyc" },
      { label: "市場列表", href: "/markets" },
      { label: "個人持倉", href: "/portfolio" },
    ],
  },
  {
    heading: "責任博弈",
    links: [
      { label: "限額設定", href: "/responsible-gaming/limits" },
      { label: "冷靜期", href: "/responsible-gaming/cooldown" },
      { label: "自我排除", href: "/responsible-gaming/self-exclusion" },
      { label: "求助資源", href: "/responsible-gaming/resources" },
    ],
  },
  {
    heading: "法律與合規",
    links: [
      { label: "服務條款", href: "/legal/terms" },
      { label: "隱私政策", href: "/legal/privacy" },
      { label: "禁止地區", href: "/legal/geo-block" },
      { label: "AML 政策", href: "/legal/aml" },
    ],
  },
  {
    heading: "支援",
    links: [
      { label: "客服中心", href: "/support" },
      { label: "常見問題", href: "/support/faq" },
      { label: "申訴流程", href: "/support/disputes" },
      { label: "聯絡我們", href: "/support/contact" },
    ],
  },
];

export interface StubPageProps {
  kicker: string;
  headline: string;
  tagline?: ReactNode;
  /** Optional secondary CTA href; defaults to "/" (home). */
  backHref?: string;
  backLabel?: string;
}

export function StubPage({ kicker, headline, tagline, backHref = "/", backLabel = "回首頁" }: StubPageProps) {
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

      <ProphTile
        surface="light"
        kicker={kicker}
        headline={headline}
        tagline={tagline ?? "這個頁面正在準備中，將在後續版本上線。感謝你的耐心。"}
        actions={
          <ProphLinkButton variant="primary" href={backHref}>
            {backLabel}
          </ProphLinkButton>
        }
      />

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
