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
  const t = await getTranslations("settlement.metadata");
  return {
    title: t("title"),
    description: t("description"),
  };
}

// SPEC-004 §2 settlement state machine — codes are not translated; labels +
// bodies come from the `settlement.stateMachine.states.<CODE>` namespace.
const STATE_CODES = ["CLOSED", "PENDING", "SETTLING", "SETTLED", "CANCELLED"] as const;
const ORACLE_ITEM_IDS = ["single", "dual", "median", "unknowable"] as const;
const ESCALATE_ITEM_IDS = ["amount", "multiple", "undecidable", "oracleDisputed"] as const;

export default async function SettlementPage() {
  const [t, sectionFooterColumns] = await Promise.all([
    getTranslations("settlement"),
    getFooterColumns(["markets", "legal", "support"]),
  ]);

  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav category={<Brand />} links={<MainNavLinks />} cta={<AuthCta />} />

      <ProphTile
        surface="light"
        headline={t("hero.headline")}
        tagline={t("hero.tagline")}
        actions={
          <>
            <ProphLinkButton variant="primary" href="/markets/settled">
              {t("hero.ctaPrimary")}
            </ProphLinkButton>
            <ProphLinkButton variant="secondary-pill" href="/support/disputes">
              {t("hero.ctaSecondary")}
            </ProphLinkButton>
          </>
        }
      />

      <section className="bg-proph-canvas-parchment">
        <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
          <header className="mb-8">
            <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
              {t("stateMachine.heading")}
            </h2>
            <p className="mt-3 max-w-3xl font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
              {t("stateMachine.intro")}
            </p>
          </header>

          <ol className="space-y-3">
            {STATE_CODES.map((code, i) => (
              <li
                key={code}
                className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-5"
              >
                <div className="flex items-baseline gap-3">
                  <span className="font-mono text-[12px] tabular-nums tracking-[-0.12px] text-proph-ink-muted-48">
                    {String(i + 1).padStart(2, "0")}
                  </span>
                  <code className="rounded-proph-sm border border-proph-divider-soft bg-proph-canvas-parchment px-2 py-0.5 font-mono text-[12px] tracking-[-0.12px] text-proph-ink">
                    {code}
                  </code>
                  <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">
                    {t(`stateMachine.states.${code}.label`)}
                  </h3>
                </div>
                <p className="mt-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                  {t(`stateMachine.states.${code}.body`)}
                </p>
              </li>
            ))}
          </ol>
        </div>
      </section>

      <section className="bg-proph-canvas">
        <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
          <header className="mb-8">
            <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
              {t("oracle.heading")}
            </h2>
            <p className="mt-3 max-w-3xl font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
              {t("oracle.intro")}
            </p>
          </header>

          <ul className="grid grid-cols-1 gap-4 md:grid-cols-2">
            {ORACLE_ITEM_IDS.map((id) => (
              <li
                key={id}
                className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
              >
                <h3 className="font-display text-[21px] font-semibold tracking-[0.231px]">
                  {t(`oracle.items.${id}.title`)}
                </h3>
                <p className="mt-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                  {t(`oracle.items.${id}.body`)}
                </p>
              </li>
            ))}
          </ul>
        </div>
      </section>

      <section className="bg-proph-canvas-parchment">
        <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
          <header className="mb-8">
            <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
              {t("dispute.heading")}
            </h2>
            <p className="mt-3 max-w-3xl font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
              {t("dispute.intro")}
            </p>
          </header>

          <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
            <article className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6">
              <h3 className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                {t("dispute.standard.title")}
              </h3>
              <p className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {t("dispute.standard.body")}
              </p>
            </article>

            <article className="rounded-proph-lg border border-proph-feedback-warning/40 bg-proph-canvas p-6">
              <h3 className="font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                {t("dispute.escalate.title")}
              </h3>
              <ul className="mt-3 space-y-1 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                {ESCALATE_ITEM_IDS.map((id) => (
                  <li key={id}>· {t(`dispute.escalate.items.${id}`)}</li>
                ))}
              </ul>
            </article>
          </div>

          <div className="mt-8">
            <ProphLinkButton variant="primary" href="/support/disputes">
              {t("dispute.cta")}
            </ProphLinkButton>
          </div>
        </div>
      </section>

      <section className="bg-proph-surface-tile-1 text-proph-on-dark">
        <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
          <h2 className="mt-2 font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
            {t("neutralityReminder.headline")}
          </h2>
          <p className="mt-4 max-w-3xl font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-body-muted">
            {t("neutralityReminder.body")}
          </p>
        </div>
      </section>

      <ProphFooter columns={sectionFooterColumns} legal={<FooterLegalRow />} />
    </main>
  );
}
