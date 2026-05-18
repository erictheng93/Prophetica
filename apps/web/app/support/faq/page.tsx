import type { Metadata } from "next";
import { getLocale, getTranslations } from "next-intl/server";
import { ProphLinkButton, ProphTile } from "@prophetica/ui";
import { isSupportedLocale } from "@/i18n/locales";
import { loadManagedFaqSections } from "@/lib/support-faq";
import type { PublicFaqSection } from "@prophetica/ts-sdk/support-faq";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("support.faq.metadata");
  return {
    title: t("title"),
    description: t("description"),
  };
}

interface FAQ {
  key: string;
}

interface Section {
  id: string;
  items: FAQ[];
}

const sections: Section[] = [
  {
    id: "account",
    items: [
      { key: "forgotPassword" },
      { key: "verificationCode" },
      { key: "frozen" },
      { key: "multiAccount" },
    ],
  },
  {
    id: "topup",
    items: [
      { key: "missingPoints" },
      { key: "cashout" },
      { key: "monthlyLimit" },
      { key: "raiseLimit" },
    ],
  },
  {
    id: "kyc",
    items: [{ key: "whyKyc" }, { key: "failed" }, { key: "documentImages" }],
  },
  {
    id: "settlement",
    items: [{ key: "oracle" }, { key: "dispute" }, { key: "delist" }],
  },
  {
    id: "rg",
    items: [
      { key: "tooMuch" },
      { key: "selfExclusion" },
      { key: "thirdParty" },
    ],
  },
  {
    id: "security",
    items: [{ key: "stolen" }, { key: "accountAccess" }],
  },
];

type FAQCopy = Awaited<ReturnType<typeof getTranslations<"support.faq">>>;

function RichAnswer({
  t,
  sectionId,
  itemKey,
}: {
  t: FAQCopy;
  sectionId: string;
  itemKey: string;
}) {
  return t.rich(`sections.${sectionId}.items.${itemKey}.a`, {
    strong: (chunks) => <strong className="font-semibold">{chunks}</strong>,
    kycPrivacyLink: (chunks) => (
      <a href="/kyc/privacy-notice" className="text-proph-primary underline">
        {chunks}
      </a>
    ),
    disputesLink: (chunks) => (
      <a href="/support/disputes" className="text-proph-primary underline">
        {chunks}
      </a>
    ),
    rgLink: (chunks) => (
      <a href="/responsible-gaming" className="text-proph-primary underline">
        {chunks}
      </a>
    ),
  });
}

function SectionBlock({ section, t }: { section: Section; t: FAQCopy }) {
  return (
    <section id={section.id} className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-12 md:px-10">
        <header className="mb-6">
          <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
            {t(`sections.${section.id}.title`)}
          </h2>
        </header>
        <ul className="space-y-3">
          {section.items.map((item, i) => (
            <li
              key={`${section.id}-${i}`}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <details className="group">
                <summary className="flex cursor-pointer list-none items-baseline justify-between gap-4">
                  <h3 className="font-display text-[21px] font-semibold leading-[1.19] tracking-[0.231px] text-proph-ink">
                    {t(`sections.${section.id}.items.${item.key}.q`)}
                  </h3>
                  <span className="shrink-0 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-48 transition-transform group-open:rotate-180">
                    v
                  </span>
                </summary>
                <div className="mt-3 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink-muted-80">
                  <RichAnswer t={t} sectionId={section.id} itemKey={item.key} />
                </div>
              </details>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

function ManagedSectionBlock({ section }: { section: PublicFaqSection }) {
  return (
    <section id={section.id} className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-12 md:px-10">
        <header className="mb-6">
          <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
            {section.title}
          </h2>
        </header>
        <ul className="space-y-3">
          {section.items.map((item) => (
            <li
              key={`${section.id}-${item.id}`}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <details className="group">
                <summary className="flex cursor-pointer list-none items-baseline justify-between gap-4">
                  <h3 className="font-display text-[21px] font-semibold leading-[1.19] tracking-[0.231px] text-proph-ink">
                    {item.question}
                  </h3>
                  <span className="shrink-0 font-text text-[14px] text-proph-ink-muted-48 transition-transform group-open:rotate-180">
                    v
                  </span>
                </summary>
                <p className="mt-3 whitespace-pre-line font-text text-[17px] leading-[1.47] text-proph-ink-muted-80">
                  {item.answer}
                </p>
              </details>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

export default async function FAQPage() {
  const [t, locale] = await Promise.all([
    getTranslations("support.faq"),
    getLocale(),
  ]);
  const faqLocale = isSupportedLocale(locale) ? locale : "zh-Hant";
  const managedSections = await loadManagedFaqSections({ locale: faqLocale });
  const renderedSections = managedSections?.length ? managedSections : null;

  return (
    <>
      <ProphTile
        surface="light"
        headline={t("hero.headline")}
        tagline={t("hero.tagline")}
      />

      {/* In-page anchor TOC */}
      <section className="bg-proph-canvas-parchment">
        <div className="mx-auto max-w-proph overflow-x-auto px-6 py-6 md:px-10">
          <ul className="flex items-center gap-2">
            {(renderedSections ?? sections).map((s) => (
              <li key={s.id}>
                <a
                  href={`#${s.id}`}
                  className="inline-flex items-center rounded-proph-pill border border-proph-hairline bg-proph-canvas px-4 py-1.5 font-text text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-ink-muted-80 transition-colors hover:border-proph-primary hover:text-proph-ink"
                >
                  {"title" in s ? s.title : t(`sections.${s.id}.title`)}
                </a>
              </li>
            ))}
          </ul>
        </div>
      </section>

      {renderedSections
        ? renderedSections.map((s) => (
            <ManagedSectionBlock key={s.id} section={s} />
          ))
        : sections.map((s) => <SectionBlock key={s.id} section={s} t={t} />)}

      <section className="bg-proph-canvas-parchment">
        <div className="mx-auto max-w-proph px-6 py-16 text-center md:px-10">
          <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
            {t("notFound.heading")}
          </h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            {t("notFound.body")}
          </p>
          <div className="mt-6 flex flex-wrap items-center justify-center gap-4">
            <ProphLinkButton variant="primary" href="/support/contact">
              {t("notFound.contactCta")}
            </ProphLinkButton>
            <ProphLinkButton variant="secondary-pill" href="/support/disputes">
              {t("notFound.disputesCta")}
            </ProphLinkButton>
          </div>
        </div>
      </section>
    </>
  );
}
