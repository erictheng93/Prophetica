import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { ProphTile } from "@prophetica/ui";
import { fetchKycOverview } from "@/lib/kyc-api";
import { Tier2LaunchClient } from "./tier-2-launch-client";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("kyc.tier2.metadata");
  return {
    title: t("title"),
    description: t("description"),
    robots: { index: false, follow: false },
  };
}

const requirementKeys = ["document", "camera", "address"] as const;

type Tier2Copy = Awaited<ReturnType<typeof getTranslations<"kyc.tier2">>>;

function RequirementsBlock({ t }: { t: Tier2Copy }) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
          {t("requirements.heading")}
        </h2>
        <ul className="grid grid-cols-1 gap-4 md:grid-cols-3">
          {requirementKeys.map((key) => (
            <li
              key={key}
              className="rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas p-6 shadow-proph-card transition-all duration-200 hover:-translate-y-1"
            >
              <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">
                {t(`requirements.items.${key}.title`)}
              </h3>
              <p className="mt-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {t(`requirements.items.${key}.body`)}
              </p>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

export default async function Tier2Page() {
  const [t, overview] = await Promise.all([
    getTranslations("kyc.tier2"),
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
      <RequirementsBlock t={t} />
      <Tier2LaunchClient country={country} />
    </>
  );
}
