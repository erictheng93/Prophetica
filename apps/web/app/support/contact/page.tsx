import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { ProphTile } from "@prophetica/ui";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("support.contact.metadata");
  return {
    title: t("title"),
    description: t("description"),
  };
}

interface Channel {
  key: "ticket" | "email" | "chat" | "phone";
}

// SPEC-012 §2 — channel matrix presented to end users.
const channels: Channel[] = [
  { key: "ticket" },
  { key: "email" },
  { key: "chat" },
  { key: "phone" },
];

type ContactCopy = Awaited<
  ReturnType<typeof getTranslations<"support.contact">>
>;

function ChannelCards({ t }: { t: ContactCopy }) {
  return (
    <section className="bg-proph-canvas">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">
            {t("channels.heading")}
          </h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            {t("channels.intro")}
          </p>
        </header>
        <ul className="grid grid-cols-1 gap-4 md:grid-cols-2">
          {channels.map((c) => (
            <li
              key={c.key}
              className="flex flex-col rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
              <div className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                {t(`channels.items.${c.key}.bestFor`)}
              </div>
              <h3 className="mt-1 font-display text-[28px] font-semibold leading-[1.14] tracking-[0.196px]">
                {t(`channels.items.${c.key}.label`)}
              </h3>
              <p className="mt-3 font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-ink-muted-80">
                {t(`channels.items.${c.key}.detail`)}
              </p>
              <dl className="mt-auto grid grid-cols-2 gap-4 border-t border-proph-divider-soft pt-4">
                <div>
                  <dt className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                    {t("channels.slaLabel")}
                  </dt>
                  <dd className="mt-1 font-text text-[14px] tracking-[-0.224px] text-proph-primary">
                    {t(`channels.items.${c.key}.sla`)}
                  </dd>
                </div>
                <div>
                  <dt className="font-text text-[12px] uppercase tracking-[-0.12px] text-proph-ink-muted-48">
                    {t("channels.hoursLabel")}
                  </dt>
                  <dd className="mt-1 font-text text-[14px] tracking-[-0.224px] text-proph-ink-muted-80">
                    {t(`channels.items.${c.key}.hours`)}
                  </dd>
                </div>
              </dl>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

function GeneralForm({ t }: { t: ContactCopy }) {
  return (
    <section className="bg-proph-canvas-parchment">
      <div className="mx-auto max-w-proph px-6 py-16 md:px-10">
        <header className="mb-8">
          <h2 className="font-display text-[40px] font-semibold leading-[1.1]">
            {t("form.heading")}
          </h2>
          <p className="mt-3 font-text text-[17px] tracking-[-0.374px] text-proph-ink-muted-80">
            {t("form.intro")}
          </p>
        </header>

        <form action="#" className="space-y-6">
          <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
            <label className="block">
              <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
                {t("form.fields.name")}
              </span>
              <input
                type="text"
                name="name"
                placeholder={t("form.fields.namePlaceholder")}
                className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
              />
            </label>
            <label className="block">
              <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
                {t("form.fields.email")}
              </span>
              <input
                type="email"
                name="email"
                placeholder="you@example.com"
                className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
              />
            </label>
          </div>

          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              {t("form.fields.subject")}
            </span>
            <input
              type="text"
              name="subject"
              placeholder={t("form.fields.subjectPlaceholder")}
              className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
            />
          </label>

          <label className="block">
            <span className="font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              {t("form.fields.body")}
            </span>
            <textarea
              name="body"
              rows={6}
              placeholder={t("form.fields.bodyPlaceholder")}
              className="mt-1 w-full rounded-proph-sm border border-proph-hairline bg-proph-canvas px-3 py-2 font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-ink placeholder:text-proph-ink-muted-48 focus:border-proph-primary-focus focus:outline-none"
            />
          </label>

          <p className="font-text text-[12px] leading-[1.3] tracking-[-0.12px] text-proph-ink-muted-48">
            {t.rich("form.privacyNote", {
              privacyLink: (chunks) => (
                <a
                  href="/legal/privacy"
                  className="text-proph-primary underline"
                >
                  {chunks}
                </a>
              ),
            })}
          </p>

          <div className="flex justify-end">
            <button
              type="submit"
              disabled
              className="inline-flex select-none items-center justify-center rounded-proph-pill bg-proph-primary px-[22px] py-[11px] font-text text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary disabled:cursor-not-allowed disabled:bg-proph-ink-muted-48"
            >
              {t("form.submit")}
            </button>
          </div>
        </form>
      </div>
    </section>
  );
}

export default async function ContactPage() {
  const t = await getTranslations("support.contact");

  return (
    <>
      <ProphTile
        surface="light"
        kicker={t("hero.kicker")}
        headline={t("hero.headline")}
        tagline={t("hero.tagline")}
      />
      <ChannelCards t={t} />
      <GeneralForm t={t} />
    </>
  );
}
