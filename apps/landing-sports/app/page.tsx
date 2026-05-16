import {
  ProphFooter,
  ProphLinkButton,
  ProphSubNav,
  ProphTile,
  type ProphFooterColumn,
} from "@prophetica/ui";

/*
 * SPEC-021 sports landing variant (variant.id "sports").
 *
 * Scaffold status: this page renders the required compliance content
 * (neutral-platform disclosure, responsible-gaming notice) inline. Once
 * packages/ui/compliance/* ships (SPEC-021 §3.1), replace the two tiles
 * below with the shared <NeutralPlatformDisclosure /> and
 * <ResponsibleGamingBanner /> so every landing variant stays consistent.
 *
 * Product posture (SPEC-009 §1): copy frames Prophetica as a neutral
 * matching platform for event prediction — never platform-as-counterparty,
 * never guaranteed returns.
 */

const footerColumns: ProphFooterColumn[] = [
  {
    heading: "玩法",
    links: [
      { label: "運動賽事市場", href: "/markets" },
      { label: "如何開始", href: "/how-it-works" },
    ],
  },
  {
    heading: "合規",
    links: [
      { label: "責任博弈", href: "/responsible-gaming" },
      { label: "服務條款", href: "/legal/terms" },
      { label: "隱私政策", href: "/legal/privacy" },
    ],
  },
];

export default function LandingSportsPage() {
  return (
    <main className="min-h-screen bg-proph-canvas">
      <p className="sr-only">prophetica sports landing — bootstrapping</p>

      <ProphSubNav
        category="Prophetica Sports"
        cta={
          <ProphLinkButton variant="primary" href="/auth/sign-in?mode=signup">
            立即註冊
          </ProphLinkButton>
        }
      />

      <ProphTile
        surface="light"
        kicker="運動賽事預測"
        headline="預測賽果，與其他玩家公平撮合"
        tagline="以站內虛擬銅幣對未公開的運動賽事結果下注，由中立撮合引擎成交。"
        actions={
          <>
            <ProphLinkButton variant="primary" href="/auth/sign-in?mode=signup">
              立即註冊
            </ProphLinkButton>
            <ProphLinkButton variant="secondary-pill" href="/how-it-works">
              了解玩法
            </ProphLinkButton>
          </>
        }
      />

      {/* SPEC-009 §1 — neutral-platform disclosure. Interim inline tile;
          migrate to <NeutralPlatformDisclosure /> per SPEC-021 §3.1. */}
      <ProphTile
        surface="dark"
        headline="平台中立，不下場對賭"
        tagline="Prophetica 僅提供撮合並收取手續費，不持有合約倉位、不做造市商。所有成交皆為玩家對玩家。"
      />

      {/* SPEC-006 — responsible-gaming notice. Interim inline tile;
          migrate to <ResponsibleGamingBanner /> per SPEC-021 §3.1. */}
      <ProphTile
        surface="dark-2"
        headline="理性娛樂，量力而為"
        tagline="站內銅幣不可兌換、提現或轉讓。如需設定限額或自我排除，請至責任博弈中心。"
        actions={
          <ProphLinkButton
            variant="secondary-pill-on-dark"
            href="/responsible-gaming"
          >
            責任博弈中心
          </ProphLinkButton>
        }
      />

      <ProphFooter
        columns={footerColumns}
        legal={
          <p>
            18 歲以上方可使用。Prophetica 為娛樂遊戲服務，非博弈、非彩券、非衍生性金融商品。
          </p>
        }
      />
    </main>
  );
}
