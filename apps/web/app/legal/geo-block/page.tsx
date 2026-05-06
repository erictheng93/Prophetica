import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { ProphFooter, ProphLinkButton, ProphSubNav, ProphTile } from "@prophetica/ui";
import { Brand, FooterLegalRow, getFooterColumns } from "@/lib/site-nav";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("legal.geoBlock.metadata");

  return {
    title: t("title"),
    description: t("description"),
    robots: { index: false, follow: false },
  };
}

export default async function GeoBlockPage() {
  const [t, footerColumns] = await Promise.all([
    getTranslations("legal.geoBlock"),
    getFooterColumns(["legal", "support"]),
  ]);

  return (
    <main className="min-h-screen bg-proph-canvas">
      <ProphSubNav
        category={<Brand />}
        cta={
          <a
            href="/support"
            className="font-text text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-ink/70 transition-colors hover:text-proph-ink"
          >
            {t("nav.supportCta")}
          </a>
        }
      />

      <ProphTile
        surface="light"
        kicker={t("hero.kicker")}
        headline={
          <span className="font-display text-[28px] font-semibold leading-[1.07] tracking-[-0.28px] sm:text-[34px] md:text-[40px] lg:text-[56px]">
            {t("hero.headline")}
          </span>
        }
        tagline={
          <>
            {t("hero.body1")}
            <br />
            {t("hero.body2")}
          </>
        }
      />

      <ProphTile
        surface="parchment"
        headline={t("howDetected.headline")}
        tagline={t("howDetected.body")}
      />

      <ProphTile
        surface="light"
        headline={t("appeal.headline")}
        tagline={t("appeal.body")}
        actions={
          <ProphLinkButton variant="primary" href="/support">
            {t("appeal.cta")}
          </ProphLinkButton>
        }
      />

      <ProphFooter columns={footerColumns} legal={<FooterLegalRow />} />
    </main>
  );
}
