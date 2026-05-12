import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { ProphTile } from "@prophetica/ui";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("responsibleGaming.selfExclusion.metadata");
  return {
    title: t("title"),
    description: t("description"),
    robots: { index: false, follow: false },
  };
}

const SCOPE_ITEM_IDS = ["freeze", "settle", "rejoin"] as const;
const TIMEOUT_OPTION_IDS = ["6m", "1y"] as const;
const PERMANENT_OPTION_IDS = ["hold", "donate"] as const;

type SECopy = Awaited<ReturnType<typeof getTranslations<"responsibleGaming.selfExclusion">>>;

export default async function SelfExclusionPage() {
  const t = await getTranslations("responsibleGaming.selfExclusion");
  return (
    <>
      <ProphTile
        surface="light"
        kicker={t("hero.kicker")}
        headline={t("hero.headline")}
        tagline={t("hero.tagline")}
      />
      <ScopeBlock t={t} />
      <TimeoutChoice t={t} />
      <PermanentSection t={t} />
    </>
  );
}

function ScopeBlock({ t }: { t: SECopy }) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">{t("scope.heading")}</h2>
        <ul className="grid grid-cols-1 gap-4 md:grid-cols-2">
          {SCOPE_ITEM_IDS.map((id) => (
            <li key={id} className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
              <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">
                {t(`scope.items.${id}.title`)}
              </h3>
              <p className="mt-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {t(`scope.items.${id}.body`)}
              </p>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

function TimeoutChoice({ t }: { t: SECopy }) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] uppercase text-proph-ink-muted-48">
            {t("timeout.kicker")}
          </span>
          <h2 className="mt-1 font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
            {t("timeout.heading")}
          </h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            {t("timeout.intro")}
          </p>
        </header>

        <form action="#">
          <fieldset disabled className="space-y-6">
            <fieldset className="grid grid-cols-1 gap-3 md:grid-cols-2">
              <legend className="sr-only">{t("timeout.legend")}</legend>
              {TIMEOUT_OPTION_IDS.map((id) => (
                <label
                  key={id}
                  className="flex cursor-pointer flex-col gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas p-5 has-[:checked]:border-proph-primary-focus has-[:checked]:bg-proph-canvas-parchment"
                >
                  <span className="flex items-center gap-3">
                    <input type="radio" name="timeout" value={id} className="h-4 w-4 accent-proph-primary" />
                    <span className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                      {t(`timeout.options.${id}.label`)}
                    </span>
                  </span>
                  <span className="font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                    {t(`timeout.options.${id}.body`)}
                  </span>
                </label>
              ))}
            </fieldset>

            <div className="flex flex-col items-start gap-3 border-t border-proph-divider-soft pt-6 md:flex-row md:items-center md:justify-between">
              <label className="flex min-h-11 items-center gap-3 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
                <input type="checkbox" required className="h-4 w-4 accent-proph-primary" />
                {t("timeout.consent")}
              </label>
              <button
                type="submit"
                disabled
                className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
              >
                {t("timeout.submit")}
              </button>
            </div>
          </fieldset>
        </form>
      </div>
    </section>
  );
}

function PermanentSection({ t }: { t: SECopy }) {
  return (
    <section className="bg-proph-surface-tile-1 text-proph-on-dark">
      <div className="mx-auto max-w-proph px-6 py-20 md:px-10">
        <header className="mb-10">
          <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] uppercase text-proph-body-muted">
            {t("permanent.kicker")}
          </span>
          <h2 className="mt-1 font-display text-[28px] sm:text-[34px] md:text-[40px] lg:text-[56px] font-semibold leading-[1.07] tracking-[-0.28px]">
            {t("permanent.headline")}
          </h2>
          <p className="mt-4 font-display text-[24px] font-light leading-[1.5]">
            {t("permanent.lead")}
          </p>
        </header>

        <form action="#">
          <fieldset disabled className="space-y-6">
            <fieldset className="grid grid-cols-1 gap-3 md:grid-cols-2">
              <legend className="font-display text-[21px] font-semibold tracking-[0.231px]">
                {t("permanent.pointsLegend")}
              </legend>
              {PERMANENT_OPTION_IDS.map((id) => (
                <label
                  key={id}
                  className="flex cursor-pointer flex-col gap-3 rounded-proph-md border border-proph-on-dark/20 p-5 has-[:checked]:border-proph-primary-on-dark"
                >
                  <span className="flex items-center gap-3">
                    <input
                      type="radio"
                      name="pointsDisposition"
                      value={id}
                      className="h-4 w-4 accent-proph-primary-on-dark"
                    />
                    <span className="font-display text-[21px] font-semibold tracking-[0.231px]">
                      {t(`permanent.options.${id}.title`)}
                    </span>
                  </span>
                  <span className="font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-body-muted">
                    {t(`permanent.options.${id}.body`)}
                  </span>
                </label>
              ))}
            </fieldset>

            <label className="flex items-start gap-3 rounded-proph-md border border-proph-on-dark/20 px-5 py-4 font-text text-[14px] leading-[1.43] tracking-[-0.224px]">
              <input type="checkbox" required className="mt-1 h-4 w-4 accent-proph-primary-on-dark" />
              <span>
                {t.rich("permanent.consent", {
                  strong: (chunks) => <strong className="font-semibold">{chunks}</strong>,
                })}
              </span>
            </label>

            <div className="flex flex-col items-start gap-3 border-t border-proph-on-dark/20 pt-6 md:flex-row md:items-center md:justify-between">
              <p className="font-text text-[14px] tracking-[-0.224px] text-proph-body-muted">
                {t("permanent.footnote")}
              </p>
              <button
                type="submit"
                disabled
                className="inline-flex select-none items-center justify-center rounded-proph-pill border border-proph-on-dark/40 bg-transparent px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-dark disabled:cursor-not-allowed disabled:opacity-60"
              >
                {t("permanent.submit")}
              </button>
            </div>
          </fieldset>
        </form>
      </div>
    </section>
  );
}
