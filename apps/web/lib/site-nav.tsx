import Link from "next/link";
import { getTranslations } from "next-intl/server";
import { ProphLinkButton } from "@prophetica/ui";

// Shared site navigation — i18n-aware version (S commit).
//
// Responsibilities split:
//   - Static href maps live in this file (route hrefs are not translated).
//   - Labels / headings come from `messages/<locale>.json` under
//     `common.nav.*` and `common.footer.columns.*`.
//   - Components (<Brand>, <MainNavLinks>, <AuthCta>, <FooterLegalRow>) are
//     async server components that resolve their copy via getTranslations.
//
// Migration note: callers stay unchanged (`<MainNavLinks />`); React 19 RSC
// renders async server components transparently. Footer-column consumers
// switch from `siteFooterColumns` (constant array) to
// `await getFooterColumns([...ids])` so the labels resolve at request time.

export interface SiteNavLink {
  label: string;
  href: string;
}

export interface SiteFooterColumn {
  heading: string;
  links: SiteNavLink[];
}

// -------------------- Static href maps --------------------

interface NavItemDef {
  /** i18n key under `common.nav` */
  key: string;
  href: string;
}

const mainNavItems: NavItemDef[] = [
  { key: "markets", href: "/markets" },
  { key: "howItWorks", href: "/how-it-works" },
  { key: "responsibleGaming", href: "/responsible-gaming" },
  { key: "support", href: "/support" },
];

interface FooterItemDef {
  /** i18n key under `common.footer.columns.<column>.items` */
  key: string;
  href: string;
}

export type FooterColumnId =
  | "about"
  | "player"
  | "markets"
  | "responsibleGaming"
  | "kyc"
  | "legal"
  | "support";

const FOOTER_ITEM_DEFS: Record<FooterColumnId, FooterItemDef[]> = {
  about: [
    { key: "platform", href: "/about" },
    { key: "matching", href: "/how-it-works" },
    { key: "settlement", href: "/settlement" },
    { key: "affiliate", href: "/affiliate" },
  ],
  player: [
    { key: "topup", href: "/topup" },
    { key: "kyc", href: "/kyc" },
    { key: "markets", href: "/markets" },
    { key: "portfolio", href: "/portfolio" },
  ],
  markets: [
    { key: "all", href: "/markets" },
    { key: "trending", href: "/markets/trending" },
    { key: "new", href: "/markets/new" },
    { key: "settled", href: "/markets/settled" },
    { key: "portfolio", href: "/portfolio" },
  ],
  responsibleGaming: [
    { key: "overview", href: "/responsible-gaming" },
    { key: "limits", href: "/responsible-gaming/limits" },
    { key: "cooldown", href: "/responsible-gaming/cooldown" },
    { key: "selfExclusion", href: "/responsible-gaming/self-exclusion" },
    { key: "resources", href: "/responsible-gaming/resources" },
  ],
  kyc: [
    { key: "overview", href: "/kyc" },
    { key: "tier1", href: "/kyc/tier-1" },
    { key: "tier2", href: "/kyc/tier-2" },
    { key: "tier3", href: "/kyc/tier-3" },
    { key: "privacyNotice", href: "/kyc/privacy-notice" },
  ],
  legal: [
    { key: "terms", href: "/legal/terms" },
    { key: "privacy", href: "/legal/privacy" },
    { key: "geoBlock", href: "/legal/geo-block" },
    { key: "aml", href: "/legal/aml" },
  ],
  support: [
    { key: "center", href: "/support" },
    { key: "faq", href: "/support/faq" },
    { key: "disputes", href: "/support/disputes" },
    { key: "contact", href: "/support/contact" },
  ],
};

/** 5-column footer used on landing and stub-page surfaces. */
export const siteFooterColumnIds: FooterColumnId[] = [
  "about",
  "player",
  "responsibleGaming",
  "legal",
  "support",
];

// -------------------- Async data helpers --------------------

export async function getFooterColumn(id: FooterColumnId): Promise<SiteFooterColumn> {
  const t = await getTranslations(`common.footer.columns.${id}`);
  return {
    heading: t("heading"),
    links: FOOTER_ITEM_DEFS[id].map(({ key, href }) => ({
      label: t(`items.${key}`),
      href,
    })),
  };
}

export async function getFooterColumns(ids: FooterColumnId[]): Promise<SiteFooterColumn[]> {
  return Promise.all(ids.map(getFooterColumn));
}

// -------------------- Components --------------------

/** Brand wordmark; goes in `ProphSubNav.category`. */
export async function Brand() {
  const t = await getTranslations("common");
  return (
    <Link href="/" className="text-proph-ink hover:opacity-80">
      {t("brand")}
    </Link>
  );
}

/** The 4 main nav anchors; goes in `ProphSubNav.links`. */
export async function MainNavLinks() {
  const t = await getTranslations("common.nav");
  return (
    <>
      {mainNavItems.map(({ key, href }) => (
        <a
          key={href}
          href={href}
          className="font-text text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-ink/70 transition-colors hover:text-proph-ink"
        >
          {t(key)}
        </a>
      ))}
    </>
  );
}

/** 登入 text link + 開始體驗 primary pill; goes in `ProphSubNav.cta`. */
export async function AuthCta() {
  const t = await getTranslations("common.auth");
  return (
    <div className="flex items-center gap-4">
      <a
        href="/auth/sign-in"
        className="font-text text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-ink/70 transition-colors hover:text-proph-ink"
      >
        {t("signIn")}
      </a>
      <ProphLinkButton variant="primary" href="/auth/sign-in">
        {t("getStarted")}
      </ProphLinkButton>
    </div>
  );
}

/**
 * Site-wide footer legal row. SPEC-006 §7.1 mandates 18+ + 援助電話 on every
 * footer; this component is the single rendering of that contract.
 */
export async function FooterLegalRow() {
  const t = await getTranslations("common.footer.legal");
  return (
    <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
      <span>{t("copyright", { year: new Date().getFullYear() })}</span>
      <span className="font-text text-[12px] leading-none tracking-[-0.12px]">
        {t("helplinePrefix")}{" "}
        <a href="tel:1925" className="text-proph-primary underline">
          1925
        </a>
        {t("helplineHours")} ·{" "}
        <a href="/legal/geo-block" className="text-proph-primary underline">
          {t("geoBlockLink")}
        </a>
      </span>
    </div>
  );
}
