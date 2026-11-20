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
  siteFooterColumnIds,
} from "@/lib/site-nav";
import { GuardianWidget } from "@/components/landing/guardian-widget";
import { HeroPredictionWidget } from "@/components/landing/hero-prediction-widget";
import { MarketTickerRibbon } from "@/components/landing/market-ticker-ribbon";
import { MatchingEngineWidget } from "@/components/landing/matching-engine-widget";
import { PointsWalletWidget } from "@/components/landing/points-wallet-widget";

export default async function LandingPage() {
  const [t, footerColumns] = await Promise.all([
    getTranslations("landing"),
    getFooterColumns(siteFooterColumnIds),
  ]);

  return (
    <main className="min-h-screen bg-proph-canvas">
      <p className="sr-only">prophetica web - bootstrapping</p>
      {/*
       * Single primary nav (parchment frosted). Replaces the prior two-row
       * stack (surface-black global-nav + parchment sub-nav). prophetica's
       * surface count doesn't justify Apple's two-tier nav model — one row is
       * enough information density for now.
       */}
      <ProphSubNav
        category={<Brand />}
        links={<MainNavLinks />}
        cta={<AuthCta />}
      />

      <ProphTile
        surface="light"
        glow="brand"
        gridPattern
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
            <ProphLinkButton variant="secondary-pill" href="/how-it-works">
              {t("hero.ctaSecondary")}
            </ProphLinkButton>
          </>
        }
        media={<HeroPredictionWidget />}
      />

      <MarketTickerRibbon />

      <ProphTile
        surface="dark"
        glow="brand"
        headline={t("neutral.headline")}
        tagline={t("neutral.tagline")}
        actions={
          <ProphLinkButton variant="primary" href="/how-it-works">
            {t("neutral.cta")}
          </ProphLinkButton>
        }
        media={<MatchingEngineWidget />}
      />

      <ProphTile
        surface="parchment"
        glow="gold"
        headline={t("points.headline")}
        tagline={t("points.tagline")}
        actions={
          <>
            <ProphLinkButton variant="primary" href="/topup">
              {t("points.ctaPrimary")}
            </ProphLinkButton>
            <ProphLinkButton variant="secondary-pill" href="/legal/terms">
              {t("points.ctaSecondary")}
            </ProphLinkButton>
          </>
        }
        media={<PointsWalletWidget />}
      />

      <ProphTile
        surface="dark-2"
        glow="emerald"
        headline={t("responsibleGaming.headline")}
        tagline={t("responsibleGaming.tagline")}
        actions={
          <>
            <ProphLinkButton
              variant="primary"
              href="/responsible-gaming/limits"
            >
              {t("responsibleGaming.ctaPrimary")}
            </ProphLinkButton>
            <ProphLinkButton
              variant="secondary-pill-on-dark"
              href="/responsible-gaming/resources"
            >
              {t("responsibleGaming.ctaSecondary")}
            </ProphLinkButton>
          </>
        }
        media={<GuardianWidget />}
      />

      <ProphTile
        surface="light"
        glow="brand"
        gridPattern
        headline={t("start.headline")}
        tagline={t("start.tagline")}
        actions={
          <>
            <ProphLinkButton variant="primary" href="/auth/sign-in?mode=signup">
              {t("start.ctaPrimary")}
            </ProphLinkButton>
            <ProphLinkButton variant="secondary-pill" href="/markets">
              {t("start.ctaSecondary")}
            </ProphLinkButton>
          </>
        }
      />

      <ProphFooter columns={footerColumns} legal={<FooterLegalRow />} />
    </main>
  );
}
