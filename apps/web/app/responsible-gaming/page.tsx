import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { ProphTile } from "@prophetica/ui";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("responsibleGaming.metadata");
  return {
    title: t("title"),
    description: t("description"),
    robots: { index: true, follow: true },
  };
}

const TOOL_IDS = ["limits", "cooldown", "selfExclusion", "resources"] as const;
const TOOL_HREFS: Record<(typeof TOOL_IDS)[number], string> = {
  limits: "/responsible-gaming/limits",
  cooldown: "/responsible-gaming/cooldown",
  selfExclusion: "/responsible-gaming/self-exclusion",
  resources: "/responsible-gaming/resources",
};

// Mock monthly snapshot — replaced by real data once wallet-ledger-service
// (SPEC-002) is wired through packages/ts-sdk.
const mockSnapshot = {
  depositPoints: 8400,
  depositLimit: 30000,
  tradeCount: 26,
  netSettlementPoints: -1240,
};


type ToolsCopy = Awaited<ReturnType<typeof getTranslations<"responsibleGaming.overview.tools">>>;
type SnapshotCopy = Awaited<
  ReturnType<typeof getTranslations<"responsibleGaming.overview.monthlySnapshot">>
>;

export default async function ResponsibleGamingPage() {
  const [tHero, tSnapshot, tTools, tFmt] = await Promise.all([
    getTranslations("responsibleGaming.overview.hero"),
    getTranslations("responsibleGaming.overview.monthlySnapshot"),
    getTranslations("responsibleGaming.overview.tools"),
    getTranslations("common.format"),
  ]);
  const formatPoints = (n: number) => tFmt("points", { n: n.toLocaleString("zh-Hant") });

  return (
    <>
      <ProphTile
        surface="light"
        kicker={tHero("kicker")}
        headline={
          <span className="font-display text-[28px] sm:text-[34px] md:text-[40px] lg:text-[56px] font-semibold leading-[1.07] tracking-[-0.28px]">
            {tHero("headline")}
          </span>
        }
        tagline={tHero("tagline")}
      />

      <MonthlySnapshot t={tSnapshot} formatPoints={formatPoints} />
      <ToolsGrid t={tTools} />
    </>
  );
}

function MonthlySnapshot({
  t,
  formatPoints,
}: {
  t: SnapshotCopy;
  formatPoints: (n: number) => string;
}) {
  const usagePct = Math.min(100, (mockSnapshot.depositPoints / mockSnapshot.depositLimit) * 100);
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] uppercase text-proph-ink-muted-48">
            {t("monthLabel")}
          </span>
          <h2 className="mt-1 font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
            {t("heading")}
          </h2>
        </header>

        <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
          <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
            <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
              {t("depositCard.kicker")}
            </div>
            <div className="mt-2 font-display text-[28px] font-semibold leading-none tracking-[-0.374px] sm:text-[34px]">
              {formatPoints(mockSnapshot.depositPoints)}
            </div>
            <div className="mt-1 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
              {t("depositCard.limitNote", { limit: formatPoints(mockSnapshot.depositLimit) })}
            </div>
            <div className="mt-4 h-1.5 w-full overflow-hidden rounded-proph-pill bg-proph-divider-soft">
              <div
                className="h-full bg-proph-primary"
                style={{ width: `${usagePct}%` }}
                aria-label={t("depositCard.ariaLabel", { pct: usagePct.toFixed(0) })}
              />
            </div>
          </article>

          <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
            <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
              {t("tradeCard.kicker")}
            </div>
            <div className="mt-2 font-display text-[28px] font-semibold leading-none tracking-[-0.374px] sm:text-[34px]">
              {mockSnapshot.tradeCount}
            </div>
            <div className="mt-1 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
              {t("tradeCard.subtitle")}
            </div>
          </article>

          <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
            <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
              {t("settlementCard.kicker")}
            </div>
            <div className="mt-2 font-display text-[28px] font-semibold leading-none tracking-[-0.374px] sm:text-[34px]">
              {formatPoints(mockSnapshot.netSettlementPoints)}
            </div>
            <div className="mt-1 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
              {t("settlementCard.subtitle")}
            </div>
          </article>
        </div>
      </div>
    </section>
  );
}

function ToolsGrid({ t }: { t: ToolsCopy }) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
          {t("heading")}
        </h2>
        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          {TOOL_IDS.map((id) => (
            <a
              key={id}
              href={TOOL_HREFS[id]}
              className="group rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6 transition-colors hover:border-proph-primary"
            >
              <span className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                {t(`items.${id}.kicker`)}
              </span>
              <h3 className="mt-1 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px] text-proph-ink">
                {t(`items.${id}.title`)}
              </h3>
              <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {t(`items.${id}.body`)}
              </p>
              <span className="mt-4 inline-block font-text text-[17px] tracking-[-0.374px] text-proph-primary group-hover:underline">
                {t("cta")} →
              </span>
            </a>
          ))}
        </div>
      </div>
    </section>
  );
}
