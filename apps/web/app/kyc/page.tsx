import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { ProphLinkButton, ProphTile } from "@prophetica/ui";
import { TierStepper } from "@/components/tier-stepper";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("kyc.overview.metadata");
  return {
    title: t("title"),
    description: t("description"),
  };
}

// Mock current state — real data from user-service / kyc-service via packages/ts-sdk.
const mockUser = {
  currentTier: 1 as const,
  thisMonthDeposit: 8400,
  monthlyLimit: 30000,
  declaredMonthlyIncome: null as number | null, // Tier 1 不申報
};

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

const formatPoints = (n: number) => `${n.toLocaleString("zh-Hant")} 點`;

type KycOverviewCopy = Awaited<
  ReturnType<typeof getTranslations<"kyc.overview">>
>;

function CurrentStatusBlock({ t }: { t: KycOverviewCopy }) {
  const usagePct = Math.min(
    100,
    (mockUser.thisMonthDeposit / mockUser.monthlyLimit) * 100,
  );

  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] uppercase text-proph-ink-muted-48">
            {t("current.kicker")}
          </span>
          <h2 className="mt-1 font-display text-[40px] font-semibold leading-[1.1]">
            {t("current.heading")}
          </h2>
        </header>

        <TierStepper currentTier={mockUser.currentTier} />

        <article className="mt-8 rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
          <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
            {t("current.depositLabel")}
          </div>
          <div className="mt-2 font-display text-[34px] font-semibold leading-none tracking-[-0.374px]">
            {formatPoints(mockUser.thisMonthDeposit)} /{" "}
            {formatPoints(mockUser.monthlyLimit)}
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

function TierCards({ t }: { t: KycOverviewCopy }) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[40px] font-semibold leading-[1.1]">
          {t("tiers.heading")}
        </h2>
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
                    <li key={r}>· {r}</li>
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
  const t = await getTranslations("kyc.overview");

  return (
    <>
      <ProphTile
        surface="light"
        kicker={t("hero.kicker")}
        headline={t("hero.headline")}
        tagline={t("hero.tagline")}
      />
      <CurrentStatusBlock t={t} />
      <TierCards t={t} />
    </>
  );
}
