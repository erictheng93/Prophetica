import Link from "next/link";
import { ProphLinkButton } from "@prophetica/ui";

// Shared site navigation data + small composable components.
//
// Why this file exists:
//   Earlier each consumer (landing / stub-page / RG layout / KYC layout)
//   declared its own `navLinks` array, `footerColumns` array, and inline
//   <Brand>/<AuthCta>/<LegalRow> JSX. Every footer-link addition meant 4
//   files to touch. This module is the single source of truth.
//
// Composition principles:
//   - Each well-known footer column is a named constant — section layouts
//     (RG, KYC) compose `[<sectionColumn>, legalColumn, supportColumn]`,
//     full-site surfaces (landing, stub-page) use `siteFooterColumns`.
//   - Brand uses next/link for client-side routing; do not downgrade to <a>.
//   - The legal row is the only footer mandate from SPEC-006 §7.1 —
//     anywhere ProphFooter is rendered, render <FooterLegalRow/> in `legal`.

export interface SiteNavLink {
  label: string;
  href: string;
}

export interface SiteFooterColumn {
  heading: string;
  links: SiteNavLink[];
}

// -------------------- Main nav (the 4 top-level links) --------------------

export const mainNavLinks: SiteNavLink[] = [
  { label: "市場", href: "/markets" },
  { label: "玩法", href: "/how-it-works" },
  { label: "責任博弈", href: "/responsible-gaming" },
  { label: "客服", href: "/support" },
];

// -------------------- Footer columns (one per section) --------------------

export const aboutColumn: SiteFooterColumn = {
  heading: "認識 prophetica",
  links: [
    { label: "關於平台", href: "/about" },
    { label: "撮合機制", href: "/how-it-works" },
    { label: "結算與爭議", href: "/settlement" },
    { label: "代理計畫", href: "/affiliate" },
  ],
};

export const playerColumn: SiteFooterColumn = {
  heading: "玩家中心",
  links: [
    { label: "如何儲值點數", href: "/topup" },
    { label: "身份驗證 KYC", href: "/kyc" },
    { label: "市場列表", href: "/markets" },
    { label: "個人持倉", href: "/portfolio" },
  ],
};

export const responsibleGamingColumn: SiteFooterColumn = {
  heading: "責任博弈",
  links: [
    { label: "概覽", href: "/responsible-gaming" },
    { label: "限額設定", href: "/responsible-gaming/limits" },
    { label: "冷靜期", href: "/responsible-gaming/cooldown" },
    { label: "自我排除", href: "/responsible-gaming/self-exclusion" },
    { label: "求助資源", href: "/responsible-gaming/resources" },
  ],
};

export const marketsColumn: SiteFooterColumn = {
  heading: "市場",
  links: [
    { label: "全部市場", href: "/markets" },
    { label: "熱門", href: "/markets/trending" },
    { label: "新上架", href: "/markets/new" },
    { label: "已結算", href: "/markets/settled" },
    { label: "個人持倉", href: "/portfolio" },
  ],
};

export const kycColumn: SiteFooterColumn = {
  heading: "身份驗證",
  links: [
    { label: "KYC 概覽", href: "/kyc" },
    { label: "Tier 1 基礎", href: "/kyc/tier-1" },
    { label: "Tier 2 標準", href: "/kyc/tier-2" },
    { label: "Tier 3 強化", href: "/kyc/tier-3" },
    { label: "隱私說明", href: "/kyc/privacy-notice" },
  ],
};

export const legalColumn: SiteFooterColumn = {
  heading: "法律與合規",
  links: [
    { label: "服務條款", href: "/legal/terms" },
    { label: "隱私政策", href: "/legal/privacy" },
    { label: "禁止地區", href: "/legal/geo-block" },
    { label: "AML 政策", href: "/legal/aml" },
  ],
};

export const supportColumn: SiteFooterColumn = {
  heading: "支援",
  links: [
    { label: "客服中心", href: "/support" },
    { label: "常見問題", href: "/support/faq" },
    { label: "申訴流程", href: "/support/disputes" },
    { label: "聯絡我們", href: "/support/contact" },
  ],
};

// Full 5-column footer used on the landing page and on /stub-page surfaces
// where the site IA should be visible at a glance.
export const siteFooterColumns: SiteFooterColumn[] = [
  aboutColumn,
  playerColumn,
  responsibleGamingColumn,
  legalColumn,
  supportColumn,
];

// -------------------- Components --------------------

/** Brand wordmark; goes in `ProphSubNav.category`. */
export function Brand() {
  return (
    <Link href="/" className="text-proph-ink hover:opacity-80">
      prophetica
    </Link>
  );
}

/** The 4 main nav anchors; goes in `ProphSubNav.links`. */
export function MainNavLinks() {
  return (
    <>
      {mainNavLinks.map((l) => (
        <a
          key={l.href}
          href={l.href}
          className="font-text text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-ink/70 transition-colors hover:text-proph-ink"
        >
          {l.label}
        </a>
      ))}
    </>
  );
}

/** 登入 text link + 開始體驗 primary pill; goes in `ProphSubNav.cta`. */
export function AuthCta() {
  return (
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
  );
}

/**
 * Site-wide footer legal row. SPEC-006 §7.1 mandates 18+ + 援助電話 on every
 * footer; this component is the single rendering of that contract.
 */
export function FooterLegalRow() {
  return (
    <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
      <span>
        © {new Date().getFullYear()} prophetica. 趣味預測娛樂遊戲服務 · 18+ ·
        站內點數不可兌回現金 / 虛擬貨幣 / 實體商品。
      </span>
      <span className="font-text text-[12px] leading-none tracking-[-0.12px]">
        援助專線{" "}
        <a href="tel:1925" className="text-proph-primary hover:underline">
          1925
        </a>
        （24 小時）·{" "}
        <a href="/legal/geo-block" className="hover:underline">
          服務地區限制
        </a>
      </span>
    </div>
  );
}
