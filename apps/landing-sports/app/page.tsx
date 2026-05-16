import type { CSSProperties, ReactNode } from "react";
import { getTranslations } from "next-intl/server";
import {
  ProphFooter,
  ProphLinkButton,
  ProphSubNav,
  ProphTile,
} from "@prophetica/ui";
import {
  LandingAnalyticsBoundary,
  TrackedSignupButton,
} from "./landing-analytics";
import {
  getSportsFooterColumns,
  SportsAuthCta,
  SportsBrand,
  SportsFooterLegalRow,
  SportsNavLinks,
} from "@/lib/site-nav";

/*
 * SPEC-021 sports landing variant (variant.id "sports").
 *
 * Product posture (SPEC-009 §1): copy frames Prophetica as a neutral
 * matching platform for event prediction - never platform-as-counterparty,
 * never guaranteed returns.
 */

const blackGoldLuxuryDesign = {
  velvetBlack: "#0A0808",
  stageBlack: "#000000",
  elevatedPanel: "#141010",
  champagneGold: "#D4A857",
  warmGold: "#F0D78C",
  smokeGold: "#C9B98F",
  pearl: "#EDE6D8",
} as const;

const goldButtonStyle: CSSProperties = {
  background:
    "linear-gradient(180deg, #F0D78C 0%, #D4A857 58%, #A68340 100%)",
  boxShadow:
    "inset 0 1px 0 rgba(255, 240, 200, 0.72), inset 0 -1px 0 rgba(80, 50, 0, 0.5), 0 0 32px rgba(212, 168, 87, 0.34)",
  color: blackGoldLuxuryDesign.stageBlack,
};

const ghostGoldClass =
  "border-[#D4A857]/70 text-[#EDE6D8] hover:bg-[#D4A857]/10 hover:text-[#F0D78C]";

const blackGoldTileClass =
  "!bg-[var(--sports-bg)] !text-[var(--sports-text)] [&_h2]:!text-[var(--sports-heading)] [&_p]:!text-[var(--sports-muted)]";

const raisedTileClass =
  "!bg-[var(--sports-bg-alt)] !text-[var(--sports-text)] [&_h2]:!text-[var(--sports-heading)] [&_p]:!text-[var(--sports-muted)]";

function GoldPrimaryCta({
  placement,
  label = "開始體驗",
}: {
  placement: string;
  label?: string;
}) {
  return (
    <TrackedSignupButton
      className="border-0 px-[28px] py-[14px] text-[16px] font-semibold leading-none transition hover:scale-[1.02]"
      label={label}
      placement={placement}
      style={goldButtonStyle}
      variant="primary"
    />
  );
}

function GoldGhostLink({
  href,
  children,
}: {
  href: string;
  children: ReactNode;
}) {
  return (
    <ProphLinkButton
      className={ghostGoldClass}
      href={href}
      variant="secondary-pill-on-dark"
    >
      {children}
    </ProphLinkButton>
  );
}

function BlackGoldTileMedia({
  label,
  details,
  variant = "sigil",
}: {
  label: string;
  details: string[];
  variant?: "sigil" | "platform" | "wallet" | "responsible";
}) {
  return (
    <div
      className="relative isolate flex h-64 w-full max-w-3xl items-center justify-center overflow-hidden rounded-[24px] border border-[var(--sports-border)] bg-[var(--sports-panel)]"
      style={{
        boxShadow:
          "inset 0 1px 0 rgba(240, 215, 140, 0.28), inset 0 -22px 54px rgba(0, 0, 0, 0.72), 0 24px 70px rgba(0, 0, 0, 0.48), 0 0 34px rgba(212, 168, 87, 0.16)",
      }}
    >
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_50%_0%,rgba(240,215,140,0.24),transparent_34%),radial-gradient(circle_at_14%_86%,rgba(212,168,87,0.15),transparent_30%)]" />
      <div className="absolute inset-x-[18%] top-0 h-full bg-[conic-gradient(from_225deg_at_50%_0%,transparent_0_18deg,rgba(240,215,140,0.16)_20deg_28deg,transparent_30deg_58deg,rgba(212,168,87,0.09)_60deg_66deg,transparent_68deg)] opacity-75 mix-blend-screen" />
      <div className="relative grid gap-5 text-center">
        <div className="mx-auto grid h-24 w-24 place-items-center rounded-[28px] border border-[#D4A857]/42 bg-[linear-gradient(145deg,#241B12,#080504)]">
          <span className="font-display text-[48px] font-semibold leading-none text-[#F0D78C]">
            S
          </span>
        </div>
        <div>
          <p className="font-display text-[21px] font-semibold leading-[1.2] tracking-[0] text-[#F0D78C]">
            {label}
          </p>
          <div className="mt-4 flex flex-wrap justify-center gap-2">
            {details.map((detail) => (
              <span
                key={detail}
                className="rounded-full border border-[var(--sports-border)] bg-[var(--sports-bg-alt)] px-3 py-1 font-text text-[12px] leading-none tracking-[0.08em] text-[var(--sports-muted)]"
              >
                {detail}
              </span>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

export default async function LandingSportsPage() {
  const [t, footerColumns] = await Promise.all([
    getTranslations("landingSports"),
    getSportsFooterColumns(),
  ]);
  const mediaTags = {
    sigil: t.raw("mediaTags.sigil") as string[],
    platform: t.raw("mediaTags.platform") as string[],
    wallet: t.raw("mediaTags.wallet") as string[],
    responsible: t.raw("mediaTags.responsible") as string[],
  };

  return (
    <LandingAnalyticsBoundary>
      <main className="min-h-screen bg-[var(--sports-bg)] text-[var(--sports-text)]">
        <p className="sr-only">prophetica sports landing - bootstrapping</p>

        <ProphSubNav
          category={<SportsBrand />}
          links={<SportsNavLinks />}
          cta={<SportsAuthCta />}
          className="!border-[#D4A857]/20 !text-[#EDE6D8] ![background:rgba(10,8,8,0.9)] [backdrop-filter:saturate(160%)_blur(18px)]"
        />

        <ProphTile
          surface="light"
          className={blackGoldTileClass}
          headline={
            <span className="font-display text-[28px] font-semibold leading-[1.07] tracking-[0] sm:text-[34px] md:text-[40px] lg:text-[56px]">
              {t("hero.headline")}
            </span>
          }
          tagline={t("hero.tagline")}
          actions={
            <>
              <GoldPrimaryCta label={t("hero.ctaPrimary")} placement="hero" />
              <GoldGhostLink href="/how-it-works">
                {t("hero.ctaSecondary")}
              </GoldGhostLink>
            </>
          }
          media={
            <BlackGoldTileMedia
              details={mediaTags.sigil}
              label={t("hero.media")}
            />
          }
        />

        <ProphTile
          surface="dark"
          className={raisedTileClass}
          headline={t("neutral.headline")}
          tagline={t("neutral.tagline")}
          actions={
            <GoldGhostLink href="/how-it-works">
              {t("neutral.cta")}
            </GoldGhostLink>
          }
          media={
            <BlackGoldTileMedia
              details={mediaTags.platform}
              label={t("neutral.media")}
              variant="platform"
            />
          }
        />

        <ProphTile
          surface="parchment"
          className={blackGoldTileClass}
          headline={t("points.headline")}
          tagline={t("points.tagline")}
          actions={
            <>
              <GoldGhostLink href="/topup">
                {t("points.ctaPrimary")}
              </GoldGhostLink>
              <GoldGhostLink href="/legal/terms">
                {t("points.ctaSecondary")}
              </GoldGhostLink>
            </>
          }
          media={
            <BlackGoldTileMedia
              details={mediaTags.wallet}
              label={t("points.media")}
              variant="wallet"
            />
          }
        />

        <ProphTile
          surface="dark-2"
          className={raisedTileClass}
          headline={t("responsibleGaming.headline")}
          tagline={t("responsibleGaming.tagline")}
          actions={
            <>
              <GoldGhostLink href="/responsible-gaming/limits">
                {t("responsibleGaming.ctaPrimary")}
              </GoldGhostLink>
              <GoldGhostLink href="/responsible-gaming/resources">
                {t("responsibleGaming.ctaSecondary")}
              </GoldGhostLink>
            </>
          }
          media={
            <BlackGoldTileMedia
              details={mediaTags.responsible}
              label={t("responsibleGaming.media")}
              variant="responsible"
            />
          }
        />

        <ProphTile
          surface="light"
          className={blackGoldTileClass}
          headline={t("start.headline")}
          tagline={t("start.tagline")}
          actions={
            <>
              <GoldPrimaryCta label={t("start.ctaPrimary")} placement="start" />
              <GoldGhostLink href="/markets">
                {t("start.ctaSecondary")}
              </GoldGhostLink>
            </>
          }
        />

        <ProphFooter
          columns={footerColumns}
          className="!bg-[#000000] !text-[#C9B98F] [&_a]:!text-[#C9B98F] [&_a:hover]:!text-[#F0D78C] [&_h3]:!text-[#F0D78C] [&>div>div:last-child]:!border-[#D4A857]/20"
          legal={<SportsFooterLegalRow />}
        />
      </main>
    </LandingAnalyticsBoundary>
  );
}
