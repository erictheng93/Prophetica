import type { Metadata } from "next";
import type { ReactNode } from "react";
import { getTranslations } from "next-intl/server";
import { ProphTile } from "@prophetica/ui";
import { fetchCooldownStatus } from "@/lib/responsible-gaming-api";
import { CooldownActivationForm } from "./cooldown-form";

export const dynamic = "force-dynamic";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("responsibleGaming.cooldown.metadata");
  return { title: t("title"), description: t("description") };
}

// Cool-off has 3 fixed durations.
const DURATION_IDS = ["24h", "7d", "30d"] as const;
const FROZEN_ITEM_IDS = ["noLogin", "noActivity", "settle", "mute"] as const;

type CooldownCopy = Awaited<
  ReturnType<typeof getTranslations<"responsibleGaming.cooldown">>
>;

export default async function CooldownPage() {
  const [t, initialStatus] = await Promise.all([
    getTranslations("responsibleGaming.cooldown"),
    fetchCooldownStatus(),
  ]);
  return (
    <>
      <ProphTile
        surface="light"
        headline={t("hero.headline")}
        tagline={t("hero.tagline")}
      />
      <FrozenWhatHappens t={t} />
      <CooldownActivationForm
        initialStatus={initialStatus}
        options={DURATION_IDS.map((id) => ({
          id,
          label: t(`choose.options.${id}.label`),
          body: t(`choose.options.${id}.body`),
        }))}
        copy={{
          heading: t("choose.heading"),
          intro: t("choose.intro"),
          legend: t("choose.legend"),
          statusLabel: t("choose.status.label"),
          statusActive: t("choose.status.active", { date: "{date}" }),
          statusInactive: t("choose.status.inactive"),
          statusFallback: t("choose.status.fallback"),
          consent: t("choose.consent"),
          submit: t("choose.submit"),
          submitPending: t("choose.submitPending"),
          success: t("choose.success"),
          genericError: t("choose.genericError"),
        }}
        callout={t.rich("choose.callout", {
          exclusionLink: (chunks: ReactNode) => (
            <a href="/responsible-gaming/self-exclusion" className="underline">
              {chunks}
            </a>
          ),
        })}
      />
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
            <li
              key={id}
              className="rounded-proph-lg border border-proph-hairline bg-proph-canvas p-6"
            >
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
