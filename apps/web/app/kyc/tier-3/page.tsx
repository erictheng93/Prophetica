import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { redirect } from "next/navigation";
import { ProphTile } from "@prophetica/ui";
import { startKycUpgrade } from "@/lib/kyc-api";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("kyc.tier3.metadata");
  return {
    title: t("title"),
    description: t("description"),
    robots: { index: false, follow: false },
  };
}

// SPEC-005 §2.1 + §6.3 — Tier 3 has higher friction. PEP-positive accounts
// are auto-routed here; UI must avoid implying the user is "suspected" of
// anything. Frame as "additional verification when activity exceeds Tier 2."

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
const timelineKeys = ["documents", "video", "review", "notification"] as const;

type Tier3Copy = Awaited<ReturnType<typeof getTranslations<"kyc.tier3">>>;

async function startTierThreeAction() {
  "use server";

  await startKycUpgrade({
    targetTier: "TIER_3",
  });
  redirect("/kyc/status");
}

function WhyHere({ t }: { t: Tier3Copy }) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="font-display text-[40px] font-semibold leading-[1.1]">
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
      </div>
    </section>
  );
}

function Requirements({ t }: { t: Tier3Copy }) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[40px] font-semibold leading-[1.1]">
          {t("requirements.heading")}
        </h2>
        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          {requirementKeys.map((key) => (
            <article
              key={key}
              className="flex flex-col rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <span className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                {t(`requirements.items.${key}.kicker`)}
              </span>
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

function StartFlow({ t }: { t: Tier3Copy }) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">
            {t("start.heading")}
          </h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            {t.rich("start.intro", {
              strong: (chunks) => (
                <strong className="font-semibold text-proph-ink">
                  {chunks}
                </strong>
              ),
            })}
          </p>
        </header>

        <form
          action={startTierThreeAction}
          className="rounded-proph-lg border border-proph-primary/30 bg-proph-canvas p-8"
        >
          <h3 className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
            {t("start.timelineHeading")}
          </h3>
          <ul className="mt-4 space-y-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
            {timelineKeys.map((key) => (
              <li key={key}>· {t(`start.timeline.${key}`)}</li>
            ))}
          </ul>

          <div className="mt-8 flex flex-col items-start gap-3 md:flex-row md:items-center md:justify-between">
            <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
              {t.rich("start.supportNote", {
                supportLink: (chunks) => (
                  <a
                    href="/support/contact"
                    className="text-proph-primary underline"
                  >
                    {chunks}
                  </a>
                ),
              })}
            </p>
            <button
              type="button"
              className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary"
            >
              {t("start.submit")}
            </button>
          </div>
        </form>
      </div>
    </section>
  );
}

export default async function Tier3Page() {
  const t = await getTranslations("kyc.tier3");

  return (
    <>
      <ProphTile
        surface="light"
        kicker={t("hero.kicker")}
        headline={t("hero.headline")}
        tagline={t("hero.tagline")}
      />
      <WhyHere t={t} />
      <Requirements t={t} />
      <StartFlow t={t} />
    </>
  );
}
