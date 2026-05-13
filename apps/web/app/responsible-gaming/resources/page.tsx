import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { ProphTile } from "@prophetica/ui";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("responsibleGaming.resources.metadata");
  return {
    title: t("title"),
    description: t("description"),
    robots: { index: true, follow: true },
  };
}

const GUIDE_IDS = ["selfAssessment", "family", "supportCs"] as const;
const GUIDE_HREFS: Record<(typeof GUIDE_IDS)[number], string> = {
  selfAssessment: "#self-assessment",
  family: "#family",
  supportCs: "/support/contact",
};

type ResourcesCopy = Awaited<ReturnType<typeof getTranslations<"responsibleGaming.resources">>>;

export default async function ResourcesPage() {
  const t = await getTranslations("responsibleGaming.resources");
  return (
    <>
      <ProphTile
        surface="light"
        headline={t("hero.headline")}
        tagline={t("hero.tagline")}
      />
      <GuideCards t={t} />
      <PrivacyNote t={t} />
    </>
  );
}

function GuideCards({ t }: { t: ResourcesCopy }) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
          {t("guides.heading")}
        </h2>
        <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
          {GUIDE_IDS.map((id) => (
            <article
              key={id}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">
                {t(`guides.items.${id}.title`)}
              </h3>
              <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {t(`guides.items.${id}.body`)}
              </p>
              <a
                href={GUIDE_HREFS[id]}
                className="mt-4 inline-block font-text text-[17px] tracking-[-0.374px] text-proph-primary underline"
              >
                {t(`guides.items.${id}.cta`)} →
              </a>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

function PrivacyNote({ t }: { t: ResourcesCopy }) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">{t("privacy.heading")}</h2>
        <p className="mt-4 max-w-3xl font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
          {t.rich("privacy.external", {
            strong: (chunks) => <strong className="font-semibold text-proph-ink">{chunks}</strong>,
          })}
        </p>
        <p className="mt-4 max-w-3xl font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
          {t.rich("privacy.internal", {
            privacyLink: (chunks) => (
              <a href="/legal/privacy" className="text-proph-primary underline">
                {chunks}
              </a>
            ),
          })}
        </p>
      </div>
    </section>
  );
}
