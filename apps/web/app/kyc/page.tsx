import type { Metadata } from "next";
import { ProphLinkButton, ProphTile } from "@prophetica/ui";
import { TierStepper } from "@/components/tier-stepper";

export const metadata: Metadata = {
  title: "身份驗證 — prophetica",
  description: "TIER_0 到 TIER_3 的分層身份驗證。完成升級以解鎖更高儲值上限。",
};

// Mock current state — real data from user-service / kyc-service via packages/ts-sdk.
const mockUser = {
  currentTier: 1 as const,
  thisMonthDeposit: 8400,
  monthlyLimit: 30000,
  declaredMonthlyIncome: null as number | null, // Tier 1 不申報
};

interface TierCardData {
  tier: 0 | 1 | 2 | 3;
  title: string;
  bullet: string;
  requirements: string[];
  unlocks: string;
  href: string;
  ctaLabel: string;
}

const tierCards: TierCardData[] = [
  {
    tier: 1,
    title: "Tier 1 基礎",
    bullet: "註冊 + 首次儲值",
    requirements: ["Email + 手機 OTP", "生日 ≥ 18 歲", "首發 5 國對應證件號（隱式驗證，不需上傳影像）"],
    unlocks: "月儲值 < TWD 30,000；單筆 < TWD 5,000",
    href: "/kyc/tier-1",
    ctaLabel: "完成 Tier 1",
  },
  {
    tier: 2,
    title: "Tier 2 標準",
    bullet: "通過 SumSub 證件驗證",
    requirements: ["證件 OCR + 自拍人臉比對", "Liveness Detection（眨眼 / 轉頭）", "地址證明（水電帳單 / 銀行對帳單）"],
    unlocks: "月儲值 < TWD 300,000（已申報月收入）；TWD 100,000（未申報）",
    href: "/kyc/tier-2",
    ctaLabel: "升級 Tier 2",
  },
  {
    tier: 3,
    title: "Tier 3 強化（EDD）",
    bullet: "高活動量或平台主動觸發",
    requirements: ["收入來源證明", "銀行對帳單", "視訊驗證", "PEP / 制裁名單篩查"],
    unlocks: "個案核定，依執照地法規",
    href: "/kyc/tier-3",
    ctaLabel: "申請 Tier 3",
  },
];

const formatPoints = (n: number) => `${n.toLocaleString("zh-Hant")} 點`;

function CurrentStatusBlock() {
  const usagePct = Math.min(100, (mockUser.thisMonthDeposit / mockUser.monthlyLimit) * 100);

  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] uppercase text-proph-ink-muted-48">
            目前狀態
          </span>
          <h2 className="mt-1 font-display text-[40px] font-semibold leading-[1.1]">
            你目前是 Tier 1 基礎驗證玩家
          </h2>
        </header>

        <TierStepper currentTier={mockUser.currentTier} />

        <article className="mt-8 rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
          <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
            本月儲值用量
          </div>
          <div className="mt-2 font-display text-[34px] font-semibold leading-none tracking-[-0.374px]">
            {formatPoints(mockUser.thisMonthDeposit)} / {formatPoints(mockUser.monthlyLimit)}
          </div>
          <div className="mt-4 h-2 w-full overflow-hidden rounded-proph-pill bg-proph-divider-soft">
            <div className="h-full bg-proph-primary" style={{ width: `${usagePct}%` }} />
          </div>
          <p className="mt-4 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
            {usagePct >= 80
              ? "你即將用完本月 Tier 1 額度。升級至 Tier 2 可解鎖更高上限。"
              : "升級下一層級可解鎖更高的月儲值與單筆上限。"}
          </p>
        </article>
      </div>
    </section>
  );
}

function TierCards() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[40px] font-semibold leading-[1.1]">下一個層級</h2>
        <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
          {tierCards.map((card) => {
            const isPast = card.tier <= mockUser.currentTier;
            return (
              <article
                key={card.tier}
                className={
                  "flex flex-col rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6 " +
                  (isPast ? "opacity-60" : "")
                }
              >
                <span className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                  {card.bullet}
                </span>
                <h3 className="mt-1 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                  {card.title}
                </h3>

                <div className="mt-4 font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                  需要
                </div>
                <ul className="mt-1 space-y-1 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                  {card.requirements.map((r) => (
                    <li key={r}>· {r}</li>
                  ))}
                </ul>

                <div className="mt-4 font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                  解鎖
                </div>
                <p className="mt-1 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                  {card.unlocks}
                </p>

                <div className="mt-6 flex-grow" />
                <ProphLinkButton
                  variant={isPast ? "secondary-pill" : "primary"}
                  href={card.href}
                  className="self-start"
                >
                  {isPast ? "查看細節" : card.ctaLabel}
                </ProphLinkButton>
              </article>
            );
          })}
        </div>
      </div>
    </section>
  );
}

export default function KYCOverviewPage() {
  return (
    <>
      <ProphTile
        surface="light"
        kicker="身份驗證"
        headline="完成驗證才能開始遊戲。"
        tagline="prophetica 採分層 KYC 設計：基礎驗證 10 秒完成、標準驗證解鎖更高上限、強化驗證適用於高活動量。每一層只收必要資訊。"
      />
      <CurrentStatusBlock />
      <TierCards />
    </>
  );
}
