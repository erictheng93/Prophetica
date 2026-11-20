import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { ProphTile } from "@prophetica/ui";
import { fetchKycOverview } from "@/lib/kyc-api";
import { Tier3LaunchClient } from "./tier-3-launch-client";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("kyc.tier3.metadata");
  return {
    title: t("title"),
    description: t("description"),
    robots: { index: false, follow: false },
  };
}

const reasonKeys = [
  "monthlyLimit",
  "singleDeposit",
  "screening",
  "compliance",
  "voluntary",
] as const;
const requirementKeys = [
  "sourceOfFunds",
  "bankStatement",
  "video",
  "pep",
] as const;

type Tier3Copy = Awaited<ReturnType<typeof getTranslations<"kyc.tier3">>>;

function WhyHere({ t }: { t: Tier3Copy }) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <article className="rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas p-8 shadow-sm">
          <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
            {t("why.heading")}
          </h2>
          <p className="mt-4 max-w-3xl font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
            {t("why.intro")}
          </p>
          <ul className="mt-6 space-y-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
            {reasonKeys.map((key) => (
              <li key={key}>· {t(`why.reasons.${key}`)}</li>
            ))}
          </ul>
        </article>
      </div>
    </section>
  );
}

function Requirements({ t }: { t: Tier3Copy }) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
          {t("requirements.heading")}
        </h2>
        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          {requirementKeys.map((key) => (
            <article
              key={key}
              className="flex flex-col rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas p-6 shadow-proph-card transition-all duration-200 hover:-translate-y-1"
            >
              <h3 className="mt-1 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                {t(`requirements.items.${key}.title`)}
              </h3>
              <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {t(`requirements.items.${key}.body`)}
              </p>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

export default async function Tier3Page() {
  const [t, overview] = await Promise.all([
    getTranslations("kyc.tier3"),
    fetchKycOverview(),
  ]);

  const country = overview.country ?? "TW";

  return (
    <>
      <ProphTile
        surface="light"
        glow="gold"
        gridPattern
        headline={t("hero.headline")}
        tagline={t("hero.tagline")}
      />
      <WhyHere t={t} />
      <Requirements t={t} />
      <Tier3LaunchClient country={country} />
    </>
  );
}
