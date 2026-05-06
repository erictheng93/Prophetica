import { getTranslations } from "next-intl/server";
import { ProphFooter, ProphLinkButton, ProphSubNav, ProphTile } from "@prophetica/ui";
import {
  AuthCta,
  Brand,
  FooterLegalRow,
  getFooterColumns,
  MainNavLinks,
  siteFooterColumnIds,
} from "@/lib/site-nav";

// Landing page — DESIGN.md (Apple-style) integration test, now i18n-ready.
// First page migrated to next-intl: copy lives in messages/zh-Hant.json
// (`landing.*` namespace). Other pages still use string literals; migrate
// per the pattern here as catalogues mature (see Tech Lead followups).
//
// Compliance posture: SPEC-001 §1.1 三原則 — copy frames the product as
// 趣味預測娛樂遊戲, never as gambling/wager; 站內點數 explicitly 不可兌回.

const TileMedia = ({ label, surface }: { label: string; surface: "light" | "dark" }) => (
  <div
    className={
      "flex h-64 w-full max-w-3xl items-center justify-center rounded-proph-lg " +
      (surface === "dark"
        ? "bg-proph-surface-tile-2 text-proph-body-muted"
        : "bg-proph-canvas-parchment text-proph-ink-muted-80")
    }
  >
    <span className="font-display text-[21px] font-semibold tracking-[0.231px]">{label}</span>
  </div>
);

export default async function LandingPage() {
  const [t, footerColumns] = await Promise.all([
    getTranslations("landing"),
    getFooterColumns(siteFooterColumnIds),
  ]);

  return (
    <main className="min-h-screen bg-proph-canvas">
      {/*
       * Single primary nav (parchment frosted). Replaces the prior two-row
       * stack (surface-black global-nav + parchment sub-nav). prophetica's
       * surface count doesn't justify Apple's two-tier nav model — one row is
       * enough information density for now.
       */}
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
          <>
            <ProphLinkButton variant="primary" href="/auth/sign-in">{t("hero.ctaPrimary")}</ProphLinkButton>
            <ProphLinkButton variant="secondary-pill" href="/how-it-works">{t("hero.ctaSecondary")}</ProphLinkButton>
          </>
        }
        media={<TileMedia label={t("hero.media")} surface="light" />}
      />

      <ProphTile
        surface="dark"
        kicker={t("neutral.kicker")}
        headline={t("neutral.headline")}
        tagline={t("neutral.tagline")}
        actions={<ProphLinkButton variant="primary" href="/how-it-works">{t("neutral.cta")}</ProphLinkButton>}
        media={<TileMedia label={t("neutral.media")} surface="dark" />}
      />

      <ProphTile
        surface="parchment"
        kicker={t("points.kicker")}
        headline={t("points.headline")}
        tagline={t("points.tagline")}
        actions={
          <>
            <ProphLinkButton variant="primary" href="/topup">{t("points.ctaPrimary")}</ProphLinkButton>
            <ProphLinkButton variant="secondary-pill" href="/legal/terms">{t("points.ctaSecondary")}</ProphLinkButton>
          </>
        }
        media={<TileMedia label={t("points.media")} surface="light" />}
      />

      <ProphTile
        surface="dark-2"
        kicker={t("responsibleGaming.kicker")}
        headline={t("responsibleGaming.headline")}
        tagline={t("responsibleGaming.tagline")}
        actions={
          <>
            <ProphLinkButton variant="primary" href="/responsible-gaming/limits">{t("responsibleGaming.ctaPrimary")}</ProphLinkButton>
            <ProphLinkButton variant="secondary-pill-on-dark" href="/responsible-gaming/resources">{t("responsibleGaming.ctaSecondary")}</ProphLinkButton>
          </>
        }
        media={<TileMedia label={t("responsibleGaming.media")} surface="dark" />}
      />

      <ProphTile
        surface="light"
        kicker={t("start.kicker")}
        headline={t("start.headline")}
        tagline={t("start.tagline")}
        actions={
          <>
            <ProphLinkButton variant="primary" href="/auth/sign-in">{t("start.ctaPrimary")}</ProphLinkButton>
            <ProphLinkButton variant="secondary-pill" href="/markets">{t("start.ctaSecondary")}</ProphLinkButton>
          </>
        }
      />

      <ProphFooter columns={footerColumns} legal={<FooterLegalRow />} />
    </main>
  );
}
