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
  const t = await getTranslations("affiliate.metadata");
  return {
    title: t("title"),
    description: t("description"),
  };
}

const REVERSAL_IDS = ["chargeback", "wash", "selfAnomaly", "selfReferral"] as const;
const TAX_INDIVIDUAL_IDS = ["tw", "threshold", "crossBorder"] as const;
const TAX_CORPORATE_IDS = ["registration", "noWithhold", "crossBorder"] as const;
const RED_FLAG_IDS = ["fees", "source", "expiry"] as const;
const EXAMPLE_CARD_IDS = ["signup", "trading", "total"] as const;

const EXAMPLE_CARD_TONE: Record<(typeof EXAMPLE_CARD_IDS)[number], "success" | "neutral"> = {
  signup: "success",
  trading: "success",
  total: "neutral",
};

export default async function AffiliatePage() {
  const [t, sectionFooterColumns] = await Promise.all([
    getTranslations("affiliate"),
    getFooterColumns(["markets", "legal", "support"]),
  ]);

  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav category={<Brand />} links={<MainNavLinks />} cta={<AuthCta />} />

      <ProphTile
        surface="light"
        kicker={t("hero.kicker")}
        headline={
          <span className="font-display text-[28px] sm:text-[34px] md:text-[40px] lg:text-[56px] font-semibold leading-[1.07] tracking-[-0.28px]">
            {t("hero.headline")}
          </span>
        }
        tagline={t("hero.tagline")}
        actions={
          <ProphLinkButton variant="primary" href="/support/contact?topic=affiliate">
            {t("hero.cta")}
          </ProphLinkButton>
        }
      />

      <section className="bg-proph-canvas-parchment">
        <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
          <header className="mb-8">
            <h2 className="font-display text-[40px] font-semibold leading-[1.1]">
              {t("commission.heading")}
            </h2>
            <p className="mt-3 max-w-3xl font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
              {t("commission.intro")}
            </p>
          </header>

          <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
            {(["signup", "trading"] as const).map((id) => (
              <article
                key={id}
                className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
              >
                <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                  {t(`commission.${id}.kicker`)}
                </div>
                <h3 className="mt-1 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                  {t(`commission.${id}.title`)}
                </h3>
                <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                  {t(`commission.${id}.body`)}
                </p>
              </article>
            ))}
          </div>
        </div>
      </section>

      <section className="bg-proph-canvas">
        <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
          <header className="mb-8">
            <h2 className="font-display text-[40px] font-semibold leading-[1.1]">
              {t("workedExample.heading")}
            </h2>
          </header>

          <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-8">
            <p className="font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
              {t("workedExample.intro")}
            </p>
            <ul className="mt-3 space-y-1 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
              <li>· {t("workedExample.scenario.kyc")}</li>
              <li>· {t("workedExample.scenario.volume")}</li>
            </ul>

            <dl className="mt-6 grid grid-cols-1 gap-4 md:grid-cols-3">
              {EXAMPLE_CARD_IDS.map((id) => {
                const isSuccess = EXAMPLE_CARD_TONE[id] === "success";
                return (
                  <div
                    key={id}
                    className={
                      "rounded-proph-md border bg-proph-canvas-parchment p-5 " +
                      (isSuccess ? "border-proph-feedback-success/30" : "border-proph-hairline")
                    }
                  >
                    <dt
                      className={
                        "font-text text-[12px] uppercase tracking-[-0.12px] " +
                        (isSuccess ? "text-proph-feedback-success" : "text-proph-ink-muted-48")
                      }
                    >
                      {t(`workedExample.cards.${id}.label`)}
                    </dt>
                    <dd className="mt-2 font-mono text-[28px] font-semibold tabular-nums tracking-[-0.374px] text-proph-ink">
                      {t(`workedExample.cards.${id}.amount`)}
                    </dd>
                    <p className="mt-2 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
                      {t(`workedExample.cards.${id}.note`)}
                    </p>
                  </div>
                );
              })}
            </dl>

            <p className="mt-6 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-48">
              {t("workedExample.privacy")}
            </p>
          </article>
        </div>
      </section>

      <section className="bg-proph-canvas-parchment">
        <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
          <header className="mb-8">
            <h2 className="font-display text-[40px] font-semibold leading-[1.1]">
              {t("reversals.heading")}
            </h2>
            <p className="mt-3 max-w-3xl font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
              {t("reversals.intro")}
            </p>
          </header>

          <ul className="space-y-3">
            {REVERSAL_IDS.map((id) => (
              <li
                key={id}
                className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-5"
              >
                <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">
                  {t(`reversals.items.${id}.title`)}
                </h3>
                <p className="mt-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                  {t(`reversals.items.${id}.body`)}
                </p>
              </li>
            ))}
          </ul>
        </div>
      </section>

      <section className="bg-proph-canvas">
        <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
          <header className="mb-8">
            <h2 className="font-display text-[40px] font-semibold leading-[1.1]">
              {t("tax.heading")}
            </h2>
            <p className="mt-3 max-w-3xl font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
              {t("tax.intro")}
            </p>
          </header>

          <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
            <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
              <h3 className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                {t("tax.individual.title")}
              </h3>
              <ul className="mt-3 space-y-1 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {TAX_INDIVIDUAL_IDS.map((id) => (
                  <li key={id}>· {t(`tax.individual.items.${id}`)}</li>
                ))}
              </ul>
            </article>

            <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
              <h3 className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                {t("tax.corporate.title")}
              </h3>
              <ul className="mt-3 space-y-1 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {TAX_CORPORATE_IDS.map((id) => (
                  <li key={id}>· {t(`tax.corporate.items.${id}`)}</li>
                ))}
              </ul>
            </article>
          </div>

          <p className="mt-6 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-48">
            {t("tax.footer")}
          </p>
        </div>
      </section>

      <section className="bg-proph-surface-tile-1 text-proph-on-dark">
        <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
          <span className="font-text text-[14px] font-semibold uppercase leading-[1.29] tracking-[-0.224px] text-proph-body-muted">
            {t("notMlm.kicker")}
          </span>
          <h2 className="mt-2 font-display text-[40px] font-semibold leading-[1.1]">
            {t("notMlm.headline")}
          </h2>
          <p className="mt-4 max-w-3xl font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-body-muted">
            {t("notMlm.body")}
          </p>
          <ul className="mt-6 space-y-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-body-muted">
            {RED_FLAG_IDS.map((id) => (
              <li key={id}>· {t(`notMlm.redFlags.${id}`)}</li>
            ))}
          </ul>
        </div>
      </section>

      <ProphTile
        surface="light"
        kicker={t("closing.kicker")}
        headline={t("closing.headline")}
        tagline={t("closing.tagline")}
        actions={
          <ProphLinkButton variant="primary" href="/support/contact?topic=affiliate">
            {t("closing.cta")}
          </ProphLinkButton>
        }
      />

      <ProphFooter columns={sectionFooterColumns} legal={<FooterLegalRow />} />
    </main>
  );
}
