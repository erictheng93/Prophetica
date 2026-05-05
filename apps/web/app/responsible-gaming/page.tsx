import type { Metadata } from "next";
import { ProphTile } from "@prophetica/ui";

export const metadata: Metadata = {
  title: "責任博弈 — prophetica",
  description: "限額設定、冷靜期、自我排除與求助資源。",
  robots: { index: true, follow: true },
};

interface SectionCard {
  href: string;
  kicker: string;
  title: string;
  body: string;
}

const sections: SectionCard[] = [
  {
    href: "/responsible-gaming/limits",
    kicker: "強制",
    title: "儲值上限",
    body: "月儲值上限是 prophetica 註冊時必設的保護線。降低立即生效；提高需 24 小時冷卻期。",
  },
  {
    href: "/responsible-gaming/cooldown",
    kicker: "可選",
    title: "冷靜期",
    body: "短期暫停遊戲。期間帳戶無法登入、儲值、交易，到期自動恢復。",
  },
  {
    href: "/responsible-gaming/self-exclusion",
    kicker: "強制可選",
    title: "自我排除",
    body: "中長期或永久停止使用本服務。永久排除不可逆，請審慎決定。",
  },
  {
    href: "/responsible-gaming/resources",
    kicker: "外部",
    title: "求助資源",
    body: "戒癮諮詢、心理衛生專線與國際支援團體。24 小時免費，獨立於 prophetica 之外。",
  },
];

// Mock monthly snapshot — replaced by real data once wallet-ledger-service
// (SPEC-002) is wired through packages/ts-sdk. Showing mock numbers in the
// shell stage helps validate the visual hierarchy without inventing language
// the SPEC hasn't approved.
const mockSnapshot = {
  monthLabel: "本月（5 月）",
  depositPoints: 8400,
  depositLimit: 30000,
  tradeCount: 26,
  netSettlementPoints: -1240,
};

const formatPoints = (n: number) => `${n.toLocaleString("zh-Hant")} 點`;

function MonthlySnapshot() {
  const usagePct = Math.min(100, (mockSnapshot.depositPoints / mockSnapshot.depositLimit) * 100);
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] uppercase text-proph-ink-muted-48">
            {mockSnapshot.monthLabel}
          </span>
          <h2 className="mt-1 font-display text-[40px] font-semibold leading-[1.1]">
            你這個月的活動快照
          </h2>
        </header>

        <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
          <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
            <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
              月儲值用量
            </div>
            <div className="mt-2 font-display text-[34px] font-semibold leading-none tracking-[-0.374px]">
              {formatPoints(mockSnapshot.depositPoints)}
            </div>
            <div className="mt-1 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
              月上限 {formatPoints(mockSnapshot.depositLimit)}
            </div>
            <div className="mt-4 h-1.5 w-full overflow-hidden rounded-proph-pill bg-proph-divider-soft">
              <div
                className="h-full bg-proph-primary"
                style={{ width: `${usagePct}%` }}
                aria-label={`月儲值用量 ${usagePct.toFixed(0)}%`}
              />
            </div>
          </article>

          <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
            <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
              交易次數
            </div>
            <div className="mt-2 font-display text-[34px] font-semibold leading-none tracking-[-0.374px]">
              {mockSnapshot.tradeCount}
            </div>
            <div className="mt-1 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
              對局成交筆數
            </div>
          </article>

          <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
            <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
              淨結算
            </div>
            <div className="mt-2 font-display text-[34px] font-semibold leading-none tracking-[-0.374px]">
              {formatPoints(mockSnapshot.netSettlementPoints)}
            </div>
            <div className="mt-1 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
              本月所有結算加總
            </div>
          </article>
        </div>
      </div>
    </section>
  );
}

function SectionGrid() {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[40px] font-semibold leading-[1.1]">
          可用工具
        </h2>
        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          {sections.map((s) => (
            <a
              key={s.href}
              href={s.href}
              className="group rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6 transition-colors hover:border-proph-primary"
            >
              <span className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                {s.kicker}
              </span>
              <h3 className="mt-1 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px] text-proph-ink">
                {s.title}
              </h3>
              <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {s.body}
              </p>
              <span className="mt-4 inline-block font-text text-[17px] tracking-[-0.374px] text-proph-primary group-hover:underline">
                前往設定 →
              </span>
            </a>
          ))}
        </div>
      </div>
    </section>
  );
}

export default function ResponsibleGamingPage() {
  return (
    <>
      <ProphTile
        surface="light"
        kicker="責任博弈"
        headline={
          <span className="font-display text-[56px] font-semibold leading-[1.07] tracking-[-0.28px]">
            你的節奏，由你掌握。
          </span>
        }
        tagline="prophetica 是娛樂，不是金融商品。我們提供工具讓你隨時調整、暫停或退出，並在你需要時提供獨立的求助資源。"
      />

      <MonthlySnapshot />
      <SectionGrid />
    </>
  );
}
