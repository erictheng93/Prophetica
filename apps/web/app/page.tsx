import { ProphFooter, ProphLinkButton, ProphSubNav, ProphTile } from "@prophetica/ui";
import { AuthCta, Brand, FooterLegalRow, MainNavLinks, siteFooterColumns } from "@/lib/site-nav";

// Landing page — DESIGN.md (Apple-style) integration test.
// Compliance posture: SPEC-001 §1.1 三原則 — copy frames the product as
// 趣味預測娛樂遊戲, never as gambling/wager; 站內點數 explicitly 不可兌回.

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
      <ProphSubNav category={<Brand />} links={<MainNavLinks />} cta={<AuthCta />} />

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

      <ProphFooter columns={siteFooterColumns} legal={<FooterLegalRow />} />
    </main>
  );
}
