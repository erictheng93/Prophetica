import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { ProphFooter, ProphLinkButton, ProphSubNav, ProphTile } from "@prophetica/ui";
import {
  AuthCta,
  Brand,
  FooterLegalRow,
  getFooterColumns,
  MainNavLinks,
} from "@/lib/site-nav";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("howItWorks.metadata");
  return {
    title: t("title"),
    description: t("description"),
  };
}

const PRINCIPLE_IDS = ["points", "orderbook", "events"] as const;
const STEP_IDS = [
  { id: "register", num: "01", href: "/auth/sign-in" },
  { id: "rg", num: "02", href: "/responsible-gaming/limits" },
  { id: "topup", num: "03", href: "/topup" },
  { id: "trade", num: "04", href: "/markets" },
] as const;
const SETTLEMENT_ITEM_IDS = ["transparent", "appealWindow", "stateMachine", "autoPayout"] as const;
const PRICING_CARD_IDS = ["cost", "winYes", "loseNo"] as const;

type PricingCardId = (typeof PRICING_CARD_IDS)[number];

const PRICING_CARD_TONE: Record<
  PricingCardId,
  { border: string; dt: string; dd: string }
> = {
  cost: {
    border: "border-proph-hairline",
    dt: "text-proph-ink-muted-48",
    dd: "text-proph-ink",
  },
  winYes: {
    border: "border-proph-feedback-success/30",
    dt: "text-proph-feedback-success",
    dd: "text-proph-feedback-success",
  },
  loseNo: {
    border: "border-proph-feedback-error/30",
    dt: "text-proph-feedback-error",
    dd: "text-proph-feedback-error",
  },
};

export default async function HowItWorksPage() {
  const [t, sectionFooterColumns] = await Promise.all([
    getTranslations("howItWorks"),
    getFooterColumns(["markets", "responsibleGaming", "legal", "support"]),
  ]);

  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav category={<Brand />} links={<MainNavLinks />} cta={<AuthCta />} />

      <ProphTile
        surface="light"
        headline={
          <span className="font-display text-[28px] sm:text-[34px] md:text-[40px] lg:text-[56px] font-semibold leading-[1.07] tracking-[-0.28px]">
            {t("hero.headline")}
          </span>
        }
        tagline={t("hero.tagline")}
        actions={
          <>
            <ProphLinkButton variant="primary" href="/auth/sign-in">
              {t("hero.ctaPrimary")}
            </ProphLinkButton>
            <ProphLinkButton variant="secondary-pill" href="/markets">
              {t("hero.ctaSecondary")}
            </ProphLinkButton>
          </>
        }
      />

      <section className="bg-proph-canvas-parchment">
        <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
          <header className="mb-8">
            <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
              {t("principles.heading")}
            </h2>
            <p className="mt-3 max-w-3xl font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
              {t("principles.intro")}
            </p>
          </header>
          <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
            {PRINCIPLE_IDS.map((id) => (
              <article
                key={id}
                className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
              >
                <h3 className="mt-1 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                  {t(`principles.items.${id}.title`)}
                </h3>
                <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                  {t(`principles.items.${id}.body`)}
                </p>
              </article>
            ))}
          </div>
        </div>
      </section>

      <section className="bg-proph-canvas">
        <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
          <header className="mb-8">
            <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
              {t("startSteps.heading")}
            </h2>
            <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
              {t("startSteps.intro")}
            </p>
          </header>
          <ol className="grid grid-cols-1 gap-4 md:grid-cols-2">
            {STEP_IDS.map((step) => (
              <li
                key={step.id}
                className="flex flex-col rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
              >
                <div className="flex items-baseline gap-3">
                  <span className="font-mono text-[28px] font-semibold tabular-nums tracking-[-0.374px] text-proph-primary">
                    {step.num}
                  </span>
                  <h3 className="font-display text-[21px] font-semibold leading-[1.19] tracking-[0.231px]">
                    {t(`startSteps.items.${step.id}.title`)}
                  </h3>
                </div>
                <p className="mt-3 flex-1 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                  {t(`startSteps.items.${step.id}.body`)}
                </p>
                <a
                  href={step.href}
                  className="mt-4 inline-block font-text text-[17px] tracking-[-0.374px] text-proph-primary underline"
                >
                  {t(`startSteps.items.${step.id}.cta`)} →
                </a>
              </li>
            ))}
          </ol>
        </div>
      </section>

      <section className="bg-proph-surface-tile-1 text-proph-on-dark">
        <div className="mx-auto max-w-proph px-6 py-20 md:px-10">
          <h2 className="mt-2 font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
            {t("matching.headline")}
          </h2>
          <p className="mt-4 max-w-3xl font-display text-[24px] font-light leading-[1.5] text-proph-body-muted">
            {t.rich("matching.lead", {
              strong: (chunks) => (
                <strong className="font-semibold text-proph-on-dark">{chunks}</strong>
              ),
            })}
          </p>

          <div className="mt-10 grid grid-cols-1 gap-6 md:grid-cols-2">
            {(["platformTakes", "platformDoesNot"] as const).map((id) => (
              <article
                key={id}
                className="rounded-proph-lg border border-proph-on-dark/15 p-6"
              >
                <h3 className="mt-1 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                  {t(`matching.${id}.title`)}
                </h3>
                <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-body-muted">
                  {t(`matching.${id}.body`)}
                </p>
              </article>
            ))}
          </div>

          <div className="mt-8">
            <ProphLinkButton variant="primary" href="/markets/wbsc-2026-tw-semifinal">
              {t("matching.cta")}
            </ProphLinkButton>
          </div>
        </div>
      </section>

      <section className="bg-proph-canvas-parchment">
        <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
          <header className="mb-8">
            <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
              {t("pricing.heading")}
            </h2>
            <p className="mt-3 max-w-3xl font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
              {t("pricing.intro")}
            </p>
          </header>

          <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-8">
            <h3 className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
              {t("pricing.exampleHeading")}
            </h3>
            <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
              {t("pricing.exampleIntro")}
            </p>

            <dl className="mt-5 grid grid-cols-1 gap-4 md:grid-cols-3">
              {PRICING_CARD_IDS.map((id) => {
                const tone = PRICING_CARD_TONE[id];
                return (
                  <div
                    key={id}
                    className={`rounded-proph-md border bg-proph-canvas-parchment p-5 ${tone.border}`}
                  >
                    <dt className={`font-text text-[12px] uppercase tracking-[-0.12px] ${tone.dt}`}>
                      {t(`pricing.cards.${id}.label`)}
                    </dt>
                    <dd
                      className={`mt-2 font-mono text-[28px] font-semibold tabular-nums tracking-[-0.374px] ${tone.dd}`}
                    >
                      {t(`pricing.cards.${id}.amount`)}
                    </dd>
                    <p className="mt-2 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                      {t(`pricing.cards.${id}.note`)}
                    </p>
                  </div>
                );
              })}
            </dl>

            <p className="mt-5 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-48">
              {t("pricing.exit")}
            </p>
          </article>
        </div>
      </section>

      <section className="bg-proph-canvas">
        <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
          <header className="mb-8">
            <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
              {t("settlement.heading")}
            </h2>
            <p className="mt-3 max-w-3xl font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
              {t("settlement.intro")}
            </p>
          </header>

          <ul className="grid grid-cols-1 gap-4 md:grid-cols-2">
            {SETTLEMENT_ITEM_IDS.map((id) => (
              <li
                key={id}
                className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
              >
                <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">
                  {t(`settlement.items.${id}.title`)}
                </h3>
                <p className="mt-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                  {t(`settlement.items.${id}.body`)}
                </p>
              </li>
            ))}
          </ul>

          <div className="mt-8">
            <ProphLinkButton variant="secondary-pill" href="/support/disputes">
              {t("settlement.cta")}
            </ProphLinkButton>
          </div>
        </div>
      </section>

      <section className="bg-proph-surface-tile-2 text-proph-on-dark">
        <div className="mx-auto max-w-proph px-6 py-20 md:px-10">
          <h2 className="mt-2 font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
            {t("compliance.headline")}
          </h2>
          <p className="mt-4 max-w-3xl font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-body-muted">
            {t("compliance.body")}
          </p>

          <div className="mt-8 flex flex-wrap items-center gap-4">
            <ProphLinkButton variant="primary" href="/responsible-gaming">
              {t("compliance.ctaPrimary")}
            </ProphLinkButton>
            <a
              href="/kyc"
              className="font-text text-[17px] tracking-[-0.374px] text-proph-primary-on-dark hover:underline"
            >
              {t("compliance.kycLink")} →
            </a>
          </div>
        </div>
      </section>

      <ProphTile
        surface="light"
        headline={t("closing.headline")}
        tagline={t("closing.tagline")}
        actions={
          <>
            <ProphLinkButton variant="primary" href="/auth/sign-in">
              {t("closing.ctaPrimary")}
            </ProphLinkButton>
            <ProphLinkButton variant="secondary-pill" href="/markets">
              {t("closing.ctaSecondary")}
            </ProphLinkButton>
          </>
        }
      />

      <ProphFooter columns={sectionFooterColumns} legal={<FooterLegalRow />} />
    </main>
  );
}
