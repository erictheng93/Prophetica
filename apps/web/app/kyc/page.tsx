import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { ProphLinkButton, ProphTile } from "@prophetica/ui";
import { TierStepper } from "@/components/tier-stepper";
import { fetchKycOverview, type KycOverview } from "@/lib/kyc-api";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("kyc.overview.metadata");
  return {
    title: t("title"),
    description: t("description"),
  };
}

interface TierCardData {
  tier: 0 | 1 | 2 | 3;
  key: "tier1" | "tier2" | "tier3";
  requirementCount: number;
  href: string;
}

const tierCards: TierCardData[] = [
  {
    tier: 1,
    key: "tier1",
    requirementCount: 3,
    href: "/kyc/tier-1",
  },
  {
    tier: 2,
    key: "tier2",
    requirementCount: 3,
    href: "/kyc/tier-2",
  },
  {
    tier: 3,
    key: "tier3",
    requirementCount: 4,
    href: "/kyc/tier-3",
  },
];


type KycOverviewCopy = Awaited<
  ReturnType<typeof getTranslations<"kyc.overview">>
>;

function CurrentStatusBlock({
  t,
  overview,
  formatPoints,
}: {
  t: KycOverviewCopy;
  overview: KycOverview;
  formatPoints: (n: number) => string;
}) {
  const thisMonthDeposit = 0;
  const monthlyLimit = overview.monthlyLimitPoints ?? 0;
  const usagePct = Math.min(
    100,
    monthlyLimit === 0 ? 0 : (thisMonthDeposit / monthlyLimit) * 100,
  );

  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] uppercase text-proph-ink-muted-48">
            {t("current.kicker")}
          </span>
          <h2 className="mt-1 font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
            {t("current.heading")}
          </h2>
        </header>

        <TierStepper currentTier={overview.currentTierNumber} />

        <article className="mt-8 rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
          <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
            {t("current.depositLabel")}
          </div>
          <div className="mt-2 font-display text-[28px] font-semibold leading-[1.1] tracking-[-0.374px] sm:text-[34px] sm:leading-none">
            {formatPoints(thisMonthDeposit)} /{" "}
            {overview.monthlyLimitPoints === null
              ? t("current.individualLimit")
              : formatPoints(monthlyLimit)}
          </div>
          <div className="mt-4 h-2 w-full overflow-hidden rounded-proph-pill bg-proph-divider-soft">
            <div
              className="h-full bg-proph-primary"
              style={{ width: `${usagePct}%` }}
            />
          </div>
          <p className="mt-4 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
            {usagePct >= 80 ? t("current.nearLimit") : t("current.upgradeHint")}
          </p>
        </article>
      </div>
    </section>
  );
}

function TierCards({
  t,
  overview,
}: {
  t: KycOverviewCopy;
  overview: KycOverview;
}) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
          {t("tiers.heading")}
        </h2>
        <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
          {tierCards.map((card) => {
            const isPast = card.tier <= overview.currentTierNumber;
            return (
              <article
                key={card.tier}
                className={
                  "flex flex-col rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6 " +
                  (isPast ? "opacity-60" : "")
                }
              >
                <span className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                  {t(`tiers.cards.${card.key}.bullet`)}
                </span>
                <h3 className="mt-1 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                  {t(`tiers.cards.${card.key}.title`)}
                </h3>

                <div className="mt-4 font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                  {t("tiers.requirementsLabel")}
                </div>
                <ul className="mt-1 space-y-1 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                  {Array.from({ length: card.requirementCount }, (_, idx) =>
                    t(`tiers.cards.${card.key}.requirements.${idx}`),
                  ).map((r) => (
                    <li key={r}>- {r}</li>
                  ))}
                </ul>

                <div className="mt-4 font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                  {t("tiers.unlocksLabel")}
                </div>
                <p className="mt-1 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                  {t(`tiers.cards.${card.key}.unlocks`)}
                </p>

                <div className="mt-6 flex-grow" />
                <ProphLinkButton
                  variant={isPast ? "secondary-pill" : "primary"}
                  href={card.href}
                  className="self-start"
                >
                  {isPast
                    ? t("tiers.pastCta")
                    : t(`tiers.cards.${card.key}.ctaLabel`)}
                </ProphLinkButton>
              </article>
            );
          })}
        </div>
      </div>
    </section>
  );
}

export default async function KYCOverviewPage() {
  const [t, tFmt, overview] = await Promise.all([
    getTranslations("kyc.overview"),
    getTranslations("common.format"),
    fetchKycOverview(),
  ]);
  const formatPoints = (n: number) => tFmt("points", { n: n.toLocaleString("zh-Hant") });

  return (
    <>
      <ProphTile
        surface="light"
        kicker={t("hero.kicker")}
        headline={t("hero.headline")}
        tagline={t("hero.tagline")}
      />
      <CurrentStatusBlock t={t} overview={overview} formatPoints={formatPoints} />
      <TierCards t={t} overview={overview} />
    </>
  );
}
