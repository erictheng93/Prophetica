import Link from "next/link";
import { ProphFooter, ProphLinkButton, ProphSubNav, ProphTile } from "@prophetica/ui";

// Landing page — DESIGN.md (Apple-style) integration test.
// Compliance posture: SPEC-001 §1.1 三原則 — copy frames the product as
// 趣味預測娛樂遊戲, never as gambling/wager; 站內點數 explicitly 不可兌回.

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

const TileMedia = ({ label, surface }: { label: string; surface: "light" | "dark" }) => (
  <div
    className={
      "flex h-64 w-full max-w-3xl items-center justify-center rounded-proph-lg " +
      (surface === "dark"
        ? "bg-proph-surface-tile-2 text-proph-body-muted"
        : "bg-proph-canvas-parchment text-proph-ink-muted-48")
    }
  >
    <span className="font-display text-[21px] font-semibold tracking-[0.231px]">{label}</span>
  </div>
);

export default function LandingPage() {
  return (
    <main className="min-h-screen bg-proph-canvas">
      {/*
       * Single primary nav (parchment frosted). Replaces the prior two-row
       * stack (surface-black global-nav + parchment sub-nav). prophetica's
       * surface count doesn't justify Apple's two-tier nav model — one row is
       * enough information density for now.
       */}
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
        kicker="趣味預測娛樂"
        headline={
          <span className="font-display text-[56px] font-semibold leading-[1.07] tracking-[-0.28px]">
            預測，是一種眼光的競技。
          </span>
        }
        tagline="用站內點數對未公開事件結果進行預測，與全球玩家比眼光。"
        actions={
          <>
            <ProphLinkButton variant="primary" href="/auth/sign-in">開始體驗</ProphLinkButton>
            <ProphLinkButton variant="secondary-pill" href="/how-it-works">了解玩法</ProphLinkButton>
          </>
        }
        media={<TileMedia label="Hero visual" surface="light" />}
      />

      <ProphTile
        surface="dark"
        kicker="平台中立"
        headline="公平撮合，平台不下場。"
        tagline="prophetica 僅提供撮合與結算服務，不持有合約倉位、不做造市商。所有對局都是玩家與玩家之間的眼光對決。"
        actions={<ProphLinkButton variant="primary" href="/how-it-works">看撮合機制</ProphLinkButton>}
        media={<TileMedia label="Order book illustration" surface="dark" />}
      />

      <ProphTile
        surface="parchment"
        kicker="點數即遊戲"
        headline="站內點數，純娛樂。"
        tagline="儲值取得的點數僅可在站內使用，不可兌回現金、虛擬貨幣或實體商品。我們是娛樂遊戲服務，不是金融商品。"
        actions={
          <>
            <ProphLinkButton variant="primary" href="/topup">點數說明</ProphLinkButton>
            <ProphLinkButton variant="secondary-pill" href="/legal/terms">服務條款</ProphLinkButton>
          </>
        }
        media={<TileMedia label="Points wallet" surface="light" />}
      />

      <ProphTile
        surface="dark-2"
        kicker="責任博弈優先"
        headline="先設限額，再開始遊戲。"
        tagline="每個帳號都必須完成限額設定。我們提供冷靜期、自我排除與求助資源，並對未成年參與採零容忍。"
        actions={
          <>
            <ProphLinkButton variant="primary" href="/responsible-gaming/limits">設定我的限額</ProphLinkButton>
            <ProphLinkButton variant="secondary-pill" href="/responsible-gaming/resources">求助資源</ProphLinkButton>
          </>
        }
        media={<TileMedia label="Responsible gaming dashboard" surface="dark" />}
      />

      <ProphTile
        surface="light"
        kicker="開始遊戲三步"
        headline="完成驗證、設定限額、瀏覽市場。"
        tagline="身份驗證確保公平與合規，限額設定保護自己的娛樂節奏，市場列表是你的競技場。"
        actions={
          <>
            <ProphLinkButton variant="primary" href="/auth/sign-in">立即註冊</ProphLinkButton>
            <ProphLinkButton variant="secondary-pill" href="/markets">查看市場</ProphLinkButton>
          </>
        }
      />

      <ProphFooter
        columns={footerColumns}
        // SPEC-006 §7.1 — site-wide footer must surface 18+ marker and a
        // 24h help-line number alongside the standard legal copy.
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
