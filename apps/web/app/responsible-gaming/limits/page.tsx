import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { ProphTile } from "@prophetica/ui";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("responsibleGaming.limits.metadata");
  return { title: t("title"), description: t("description") };
}

// SPEC-006 §3.1 — fixed monthly limit options. "無上限" intentionally absent.
const limitOptions = [
  { value: 1000 },
  { value: 3000 },
  { value: 5000 },
  { value: 10000 },
  { value: 30000 },
  { value: 50000 },
  { value: 100000 },
  { value: 300000 },
  { value: 500000 },
];

const mockState = {
  currentLimit: 30000,
  thisMonthUsed: 8400,
  lastChangeAt: "2026-04-12",
};


type LimitsCopy = Awaited<ReturnType<typeof getTranslations<"responsibleGaming.limits">>>;

export default async function LimitsPage() {
  const [t, tFmt] = await Promise.all([
    getTranslations("responsibleGaming.limits"),
    getTranslations("common.format"),
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
      <CurrentLimitCard t={t} formatPoints={formatPoints} />
      <AdjustForm t={t} formatPoints={formatPoints} />
    </>
  );
}

function CurrentLimitCard({
  t,
  formatPoints,
}: {
  t: LimitsCopy;
  formatPoints: (n: number) => string;
}) {
  const usagePct = Math.min(100, (mockState.thisMonthUsed / mockState.currentLimit) * 100);
  const nearLimit = usagePct >= 80;

  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-8">
          <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
            {t("current.label")}
          </div>
          <div className="mt-2 font-display text-[28px] sm:text-[34px] md:text-[40px] lg:text-[56px] font-semibold leading-none tracking-[-0.28px]">
            {formatPoints(mockState.currentLimit)}
          </div>
          <div className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            {t("current.usage", {
              used: formatPoints(mockState.thisMonthUsed),
              pct: usagePct.toFixed(0),
              date: mockState.lastChangeAt,
            })}
          </div>

          <div className="mt-6 h-2 w-full overflow-hidden rounded-proph-pill bg-proph-divider-soft">
            <div className="h-full bg-proph-primary" style={{ width: `${usagePct}%` }} />
          </div>

          {nearLimit ? (
            <p className="mt-6 rounded-proph-md border border-proph-primary bg-proph-canvas-parchment px-4 py-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-primary">
              {t("current.warning", { pct: usagePct.toFixed(0) })}
            </p>
          ) : null}
        </article>
      </div>
    </section>
  );
}

function AdjustForm({
  t,
  formatPoints,
}: {
  t: LimitsCopy;
  formatPoints: (n: number) => string;
}) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">{t("form.heading")}</h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            {t("form.intro")}
          </p>
        </header>

        <form action="#">
          <fieldset disabled className="space-y-6">
            <fieldset className="grid grid-cols-1 gap-3 md:grid-cols-2">
              <legend className="sr-only">{t("form.legend")}</legend>
              {limitOptions.map((opt) => (
                <label
                  key={opt.value}
                  className="flex cursor-pointer items-center gap-3 rounded-proph-md border border-proph-hairline bg-proph-canvas px-5 py-4 transition-colors has-[:checked]:border-proph-primary-focus has-[:checked]:bg-proph-canvas-parchment"
                >
                  <input
                    type="radio"
                    name="monthlyLimit"
                    value={opt.value}
                    defaultChecked={opt.value === mockState.currentLimit}
                    className="h-4 w-4 accent-proph-primary"
                  />
                  <span className="font-display text-[21px] font-semibold tracking-[0.231px]">
                    {formatPoints(opt.value)}
                  </span>
                </label>
              ))}
              <label className="flex cursor-pointer flex-col gap-2 rounded-proph-md border border-proph-hairline bg-proph-canvas px-5 py-4 has-[:checked]:border-proph-primary-focus has-[:checked]:bg-proph-canvas-parchment md:col-span-2">
                <span className="flex items-center gap-3">
                  <input type="radio" name="monthlyLimit" value="custom" className="h-4 w-4 accent-proph-primary" />
                  <span className="font-display text-[21px] font-semibold tracking-[0.231px]">
                    {t("form.customLabel")}
                  </span>
                </span>
                <input
                  type="number"
                  inputMode="numeric"
                  min={1}
                  placeholder={t("form.customPlaceholder")}
                  className="ml-7 w-full min-h-11 max-w-xs rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
                  aria-label={t("form.customAriaLabel")}
                />
              </label>
            </fieldset>

            <div className="flex flex-col items-start gap-3 border-t border-proph-divider-soft pt-6 md:flex-row md:items-center md:justify-between">
              <p className="font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
                {t("form.footnote")}
              </p>
              <button
                type="submit"
                disabled
                className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
              >
                {t("form.submit")}
              </button>
            </div>
          </fieldset>
        </form>
      </div>
    </section>
  );
}
