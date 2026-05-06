import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { ProphTile } from "@prophetica/ui";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("kyc.tier2.metadata");
  return {
    title: t("title"),
    description: t("description"),
    robots: { index: false, follow: false },
  };
}

// SPEC-005 §2.1.1 — declared_monthly_income optional at Tier 2 (NULL allowed
// per fallback rule). UI presents 4 income brackets (區間中位數 mapping is
// done server-side — UI shows ranges only).
const incomeBuckets = [
  { value: "lt_30k", key: "lt30k" },
  { value: "30k_60k", key: "30k60k" },
  { value: "60k_100k", key: "60k100k" },
  { value: "gt_100k", key: "gt100k" },
] as const;

const requirementKeys = ["document", "camera", "address"] as const;

type Tier2Copy = Awaited<ReturnType<typeof getTranslations<"kyc.tier2">>>;

function RequirementsBlock({ t }: { t: Tier2Copy }) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[40px] font-semibold leading-[1.1]">
          {t("requirements.heading")}
        </h2>
        <ul className="grid grid-cols-1 gap-4 md:grid-cols-3">
          {requirementKeys.map((key) => (
            <li
              key={key}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
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

function SDKPlaceholder({ t }: { t: Tier2Copy }) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">
            {t("sumsub.heading")}
          </h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            {t("sumsub.intro")}
          </p>
        </header>

        {/*
         * Placeholder for the SumSub Web SDK iframe (SPEC-005 §3.2). Real
         * integration requires:
         *   1. Backend kyc-service exchanges short-lived applicant token via
         *      SumSub API and returns to the client.
         *   2. <SumsubWebSdk accessToken={...} onMessage={...} /> mounts here.
         *   3. webhook /api/internal/webhooks/sumsub flips DB tier on review
         *      result (§3.3).
         * Frontend integration tracked under ADR-0004 §10 V-1.
         */}
        <div className="flex h-[480px] flex-col items-center justify-center gap-3 rounded-proph-lg border-2 border-dashed border-proph-hairline bg-proph-canvas-parchment text-center">
          <span className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
            {t("sumsub.placeholderKicker")}
          </span>
          <span className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px] text-proph-ink-muted-80">
            {t("sumsub.placeholderTitle")}
          </span>
          <button
            type="button"
            disabled
            className="mt-2 inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
          >
            {t("sumsub.submit")}
          </button>
        </div>
      </div>
    </section>
  );
}

function IncomeDeclaration({ t }: { t: Tier2Copy }) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] uppercase text-proph-ink-muted-48">
            {t("income.kicker")}
          </span>
          <h2 className="mt-1 font-display text-[40px] font-semibold leading-[1.1]">
            {t("income.heading")}
          </h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            {t.rich("income.intro", {
              strong: (chunks) => (
                <strong className="font-semibold text-proph-ink">
                  {chunks}
                </strong>
              ),
            })}
          </p>
        </header>

        <form action="#">
          <fieldset disabled className="space-y-6">
            <fieldset className="grid grid-cols-1 gap-3 md:grid-cols-2">
              <legend className="sr-only">{t("income.legend")}</legend>
              {incomeBuckets.map((b) => (
                <label
                  key={b.value}
                  className="flex cursor-pointer items-center gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas px-5 py-4 has-[:checked]:border-proph-primary-focus has-[:checked]:bg-proph-canvas-parchment"
                >
                  <input
                    type="radio"
                    name="incomeBucket"
                    value={b.value}
                    className="h-4 w-4 accent-proph-primary"
                  />
                  <span className="font-display text-[21px] font-semibold tracking-[0.231px]">
                    {t(`income.buckets.${b.key}`)}
                  </span>
                </label>
              ))}
              <label className="flex cursor-pointer items-center gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas px-5 py-4 has-[:checked]:border-proph-primary-focus has-[:checked]:bg-proph-canvas-parchment md:col-span-2">
                <input
                  type="radio"
                  name="incomeBucket"
                  value="skip"
                  className="h-4 w-4 accent-proph-primary"
                />
                <span className="font-display text-[21px] font-semibold tracking-[0.231px]">
                  {t("income.skipLabel")}
                </span>
                <span className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
                  {t("income.skipHint")}
                </span>
              </label>
            </fieldset>

            <div className="flex justify-end">
              <button
                type="submit"
                disabled
                className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
              >
                {t("income.submit")}
              </button>
            </div>
          </fieldset>
        </form>
      </div>
    </section>
  );
}

export default async function Tier2Page() {
  const t = await getTranslations("kyc.tier2");

  return (
    <>
      <ProphTile
        surface="light"
        kicker={t("hero.kicker")}
        headline={t("hero.headline")}
        tagline={t("hero.tagline")}
      />
      <RequirementsBlock t={t} />
      <SDKPlaceholder t={t} />
      <IncomeDeclaration t={t} />
    </>
  );
}
