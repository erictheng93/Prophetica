import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { ProphTile } from "@prophetica/ui";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("responsibleGaming.cooldown.metadata");
  return { title: t("title"), description: t("description") };
}

// SPEC-006 §5.1 — Cool-off has 3 fixed durations.
const DURATION_IDS = ["24h", "7d", "30d"] as const;
const FROZEN_ITEM_IDS = ["noLogin", "noActivity", "settle", "mute"] as const;

type CooldownCopy = Awaited<ReturnType<typeof getTranslations<"responsibleGaming.cooldown">>>;

export default async function CooldownPage() {
  const t = await getTranslations("responsibleGaming.cooldown");
  return (
    <>
      <ProphTile
        surface="light"
        kicker={t("hero.kicker")}
        headline={t("hero.headline")}
        tagline={t("hero.tagline")}
      />
      <FrozenWhatHappens t={t} />
      <ChooseDuration t={t} />
    </>
  );
}

function FrozenWhatHappens({ t }: { t: CooldownCopy }) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mb-8 font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
          {t("frozen.heading")}
        </h2>
        <ul className="grid grid-cols-1 gap-4 md:grid-cols-2">
          {FROZEN_ITEM_IDS.map((id) => (
            <li key={id} className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
              <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">
                {t(`frozen.items.${id}.title`)}
              </h3>
              <p className="mt-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {t(`frozen.items.${id}.body`)}
              </p>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

function ChooseDuration({ t }: { t: CooldownCopy }) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">{t("choose.heading")}</h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            {t("choose.intro")}
          </p>
        </header>

        <form action="#">
          <fieldset disabled className="space-y-6">
            <fieldset className="grid grid-cols-1 gap-3 md:grid-cols-3">
              <legend className="sr-only">{t("choose.legend")}</legend>
              {DURATION_IDS.map((id) => (
                <label
                  key={id}
                  className="flex cursor-pointer flex-col gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas p-5 transition-colors has-[:checked]:border-proph-primary-focus has-[:checked]:bg-proph-canvas-parchment"
                >
                  <span className="flex items-center gap-3">
                    <input type="radio" name="duration" value={id} className="h-4 w-4 accent-proph-primary" />
                    <span className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                      {t(`choose.options.${id}.label`)}
                    </span>
                  </span>
                  <span className="font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                    {t(`choose.options.${id}.body`)}
                  </span>
                </label>
              ))}
            </fieldset>

            <div className="rounded-proph-md border border-proph-primary/30 bg-proph-canvas-parchment px-5 py-4">
              <p className="font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink">
                {t.rich("choose.callout", {
                  exclusionLink: (chunks) => (
                    <a href="/responsible-gaming/self-exclusion" className="underline">
                      {chunks}
                    </a>
                  ),
                })}
              </p>
            </div>

            <div className="flex flex-col items-start gap-3 border-t border-proph-divider-soft pt-6 md:flex-row md:items-center md:justify-between">
              <label className="flex min-h-11 items-center gap-3 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
                <input type="checkbox" required className="h-4 w-4 accent-proph-primary" />
                {t("choose.consent")}
              </label>
              <button
                type="submit"
                disabled
                className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
              >
                {t("choose.submit")}
              </button>
            </div>
          </fieldset>
        </form>
      </div>
    </section>
  );
}
