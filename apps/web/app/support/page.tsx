import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { ProphLinkButton, ProphTile } from "@prophetica/ui";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("support.overview.metadata");
  return {
    title: t("title"),
    description: t("description"),
  };
}

interface ChannelCard {
  key: "ticket" | "email" | "chat" | "phone";
}

// SPEC-012 §2 — channel matrix.
const channels: ChannelCard[] = [
  { key: "ticket" },
  { key: "email" },
  { key: "chat" },
  { key: "phone" },
];

const categories = [
  { key: "account", route: "/support/faq#account" },
  { key: "topup", route: "/support/faq#topup" },
  { key: "kyc", route: "/support/faq#kyc" },
  { key: "settlement", route: "/support/disputes" },
  { key: "security", route: "/support/contact" },
  { key: "rg", route: "/responsible-gaming" },
] as const;

type SupportOverviewCopy = Awaited<
  ReturnType<typeof getTranslations<"support.overview">>
>;

function ChannelGrid({ t }: { t: SupportOverviewCopy }) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
            {t("channels.heading")}
          </h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            {t("channels.intro")}
          </p>
        </header>
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-4">
          {channels.map((c) => (
            <article
              key={c.key}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                {t(`channels.items.${c.key}.hours`)}
              </div>
              <h3 className="mt-1 font-display text-[21px] font-semibold tracking-[0.231px]">
                {t(`channels.items.${c.key}.label`)}
              </h3>
              <div className="mt-2 font-text text-[14px] tracking-[-0.224px] text-proph-primary">
                {t(`channels.items.${c.key}.sla`)}
              </div>
              <p className="mt-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                {t(`channels.items.${c.key}.desc`)}
              </p>
            </article>
          ))}
        </div>
      </div>
    </section>
  );
}

function CategoryGrid({ t }: { t: SupportOverviewCopy }) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
            {t("categories.heading")}
          </h2>
        </header>
        <div className="grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-3">
          {categories.map((c) => (
            <a
              key={c.key}
              href={c.route}
              className="group rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6 transition-colors hover:border-proph-primary"
            >
              <h3 className="mt-1 font-display text-[21px] font-semibold tracking-[0.231px] text-proph-ink group-hover:underline">
                {t(`categories.items.${c.key}.title`)}
              </h3>
              <p className="mt-2 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                {t(`categories.items.${c.key}.body`)}
              </p>
            </a>
          ))}
        </div>
      </div>
    </section>
  );
}

function RGCallout({ t }: { t: SupportOverviewCopy }) {
  // SPEC-012 §6.2 — Help Center must surface a prominent RG help entry.
  return (
    <section className="bg-proph-surface-tile-1 text-proph-on-dark">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <h2 className="mt-2 font-display text-[28px] font-semibold leading-[1.1] sm:text-[34px] md:text-[40px]">
          {t("rgCallout.headline")}
        </h2>
        <p className="mt-4 max-w-3xl font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-body-muted">
          {t("rgCallout.body")}
        </p>
        <div className="mt-8 flex flex-wrap items-center gap-4">
          <ProphLinkButton variant="primary" href="/responsible-gaming">
            {t("rgCallout.cta")}
          </ProphLinkButton>
        </div>
      </div>
    </section>
  );
}

export default async function SupportHubPage() {
  const t = await getTranslations("support.overview");

  return (
    <>
      <ProphTile
        surface="light"
        headline={t("hero.headline")}
        tagline={t("hero.tagline")}
      />
      <ChannelGrid t={t} />
      <CategoryGrid t={t} />
      <RGCallout t={t} />
    </>
  );
}
