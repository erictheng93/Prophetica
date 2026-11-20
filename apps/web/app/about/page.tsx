import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import {
  ProphFooter,
  ProphLinkButton,
  ProphSubNav,
  ProphTile,
} from "@prophetica/ui";
import {
  AuthCta,
  Brand,
  FooterLegalRow,
  getFooterColumns,
  MainNavLinks,
} from "@/lib/site-nav";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("about.metadata");
  return {
    title: t("title"),
    description: t("description"),
  };
}

const PRINCIPLE_IDS = ["points", "orderbook", "events"] as const;
const NEUTRALITY_ITEM_IDS = ["oracle", "platformBan", "auditable"] as const;
const COMPANY_IDS = ["holding", "op", "tech"] as const;
const PRODUCT_SCOPE_ITEM_IDS = ["points", "orderbook", "records"] as const;

export default async function AboutPage() {
  const [t, sectionFooterColumns] = await Promise.all([
    getTranslations("about"),
    getFooterColumns(["markets", "responsibleGaming", "legal", "support"]),
  ]);

  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav
        category={<Brand />}
        links={<MainNavLinks />}
        cta={<AuthCta />}
      />

      <ProphTile
        surface="light"
        glow="gold"
        gridPattern
        headline={
          <span className="font-display text-[28px] sm:text-[34px] md:text-[40px] lg:text-[56px] font-semibold leading-[1.07] tracking-[-0.28px]">
            {t("hero.headline")}
          </span>
        }
        tagline={t("hero.tagline")}
        actions={
          <>
            <ProphLinkButton variant="primary" href="/how-it-works">
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
                className="rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas p-6 shadow-proph-card transition-all duration-200 hover:-translate-y-1"
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

      <section className="bg-proph-surface-tile-1 text-proph-on-dark">
        <div className="mx-auto max-w-proph px-6 py-20 md:px-10">
          <h2 className="mt-2 font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
            {t("neutrality.headline")}
          </h2>
          <p className="mt-4 max-w-3xl font-display text-[24px] font-light leading-[1.5] text-proph-body-muted">
            {t.rich("neutrality.lead", {
              highlight: (chunks) => (
                <strong className="font-semibold text-proph-on-dark">
                  {chunks}
                </strong>
              ),
            })}
          </p>

          <div className="mt-10 grid grid-cols-1 gap-6 md:grid-cols-3">
            {NEUTRALITY_ITEM_IDS.map((id) => (
              <article
                key={id}
                className="rounded-proph-xl border border-white/15 bg-white/[0.03] p-6 shadow-proph-card backdrop-blur-sm transition-all duration-200 hover:-translate-y-1"
              >
                <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">
                  {t(`neutrality.items.${id}.title`)}
                </h3>
                <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-body-muted">
                  {t(`neutrality.items.${id}.body`)}
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
              {t("structure.heading")}
            </h2>
            <p className="mt-3 max-w-3xl font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
              {t.rich("structure.intro", {
                adrLink: (chunks) => (
                  <a
                    href="/docs/architecture/decisions/ADR-0002"
                    className="text-proph-primary underline"
                  >
                    {chunks}
                  </a>
                ),
              })}
            </p>
          </header>
          <ul className="space-y-4">
            {COMPANY_IDS.map((id) => (
              <li
                key={id}
                className="rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas p-6 shadow-proph-card transition-all duration-200 hover:-translate-y-1"
              >
                <div className="flex flex-wrap items-baseline justify-between gap-3">
                  <div className="flex items-baseline gap-3">
                    <h3 className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                      {t(`structure.companies.${id}.name`)}
                    </h3>
                    <span className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                      {t(`structure.companies.${id}.role`)}
                    </span>
                  </div>
                  <span className="rounded-proph-pill border border-proph-hairline bg-proph-canvas-parchment px-3 py-0.5 font-text text-[12px] tracking-[-0.12px] text-proph-ink-muted-80">
                    {t(`structure.companies.${id}.location`)}
                  </span>
                </div>
                <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                  {t(`structure.companies.${id}.body`)}
                </p>
              </li>
            ))}
          </ul>
        </div>
      </section>

      <section className="bg-proph-canvas-parchment">
        <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
          <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
            {t("productScope.heading")}
          </h2>
          <p className="mt-4 max-w-3xl font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
            {t("productScope.intro")}
          </p>

          <ul className="mt-8 space-y-3">
            {PRODUCT_SCOPE_ITEM_IDS.map((id) => (
              <li
                key={id}
                className="rounded-proph-xl border border-proph-hairline/80 bg-proph-canvas p-5 shadow-sm transition-all duration-200 hover:-translate-y-0.5"
              >
                <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">
                  {t(`productScope.items.${id}.title`)}
                </h3>
                <p className="mt-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                  {t(`productScope.items.${id}.body`)}
                </p>
              </li>
            ))}
          </ul>

          <p className="mt-6 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48">
            {t("productScope.footer")}
          </p>
        </div>
      </section>

      <ProphTile
        surface="light"
        glow="gold"
        gridPattern
        headline={t("contact.headline")}
        tagline={t("contact.tagline")}
        actions={
          <>
            <ProphLinkButton variant="primary" href="/support/contact">
              {t("contact.ctaPrimary")}
            </ProphLinkButton>
            <ProphLinkButton variant="secondary-pill" href="/affiliate">
              {t("contact.ctaSecondary")}
            </ProphLinkButton>
          </>
        }
      />

      <ProphFooter columns={sectionFooterColumns} legal={<FooterLegalRow />} />
    </main>
  );
}
