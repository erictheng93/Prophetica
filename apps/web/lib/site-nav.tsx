import Link from "next/link";
import { getTranslations } from "next-intl/server";
import { ProphLinkButton } from "@prophetica/ui";
import { LanguageSwitcher } from "@/components/language-switcher";
import { ThemeToggle, type ThemeToggleLabels } from "@/components/theme-toggle";
import { getViewer } from "@/lib/session";

// Shared site navigation — i18n-aware version (S commit).
//
// Responsibilities split:
//   - Static href maps live in this file (route hrefs are not translated).
//   - Labels / headings come from `messages/<locale>.json` under
//     `common.nav.*` and `common.footer.columns.*`.
//   - Components (<Brand>, <MainNavLinks>, <AuthCta>, <SiteThemeToggle>,
//     <FooterLegalRow>) are async server components that resolve their copy
//     via getTranslations.
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

export async function getFooterColumn(
  id: FooterColumnId,
): Promise<SiteFooterColumn> {
  const t = await getTranslations(`common.footer.columns.${id}`);
  return {
    heading: t("heading"),
    links: FOOTER_ITEM_DEFS[id].map(({ key, href }) => ({
      label: t(`items.${key}`),
      href,
    })),
  };
}

export async function getFooterColumns(
  ids: FooterColumnId[],
): Promise<SiteFooterColumn[]> {
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
          className="font-text text-[15px] leading-[1.35] tracking-[0] text-proph-ink/70 transition-colors hover:text-proph-ink"
        >
          {t(key)}
        </a>
      ))}
    </>
  );
}

/** Account status + auth entry points; goes in `ProphSubNav.cta`. */
export async function AuthCta() {
  const [t, tPlayerNav, viewer] = await Promise.all([
    getTranslations("common.auth"),
    getTranslations("common.footer.columns.player.items"),
    getViewer(),
  ]);

  return (
    <div className="flex items-center gap-2 sm:gap-3">
      <LanguageSwitcher />
      <nav
        aria-label={t("account.ariaLabel")}
        className="flex items-center gap-2"
      >
        <span className="inline-flex min-h-8 items-center gap-2 rounded-proph-pill border border-proph-hairline bg-proph-canvas px-3 font-text text-[13px] leading-[1.31] tracking-[0] text-proph-ink-muted-80">
          <span
            className={
              "h-1.5 w-1.5 rounded-proph-full " +
              (viewer ? "bg-proph-feedback-success" : "bg-proph-ink-muted-48")
            }
            aria-hidden="true"
          />
          {viewer ? viewer.userId : t("account.status.unauthenticated")}
        </span>
        {viewer ? (
          <ProphLinkButton
            variant="primary"
            href="/portfolio"
            className="px-4 py-2 text-[14px] leading-[1.29] tracking-[-0.224px]"
          >
            {tPlayerNav("portfolio")}
          </ProphLinkButton>
        ) : (
          <>
            <a
              href="/auth/sign-in"
              className="font-text text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-ink/70 transition-colors hover:text-proph-ink"
            >
              {t("signIn")}
            </a>
            <ProphLinkButton
              variant="primary"
              href="/auth/sign-in?mode=signup"
              className="px-4 py-2 text-[14px] leading-[1.29] tracking-[-0.224px]"
            >
              {t("signUp")}
            </ProphLinkButton>
          </>
        )}
      </nav>
      <SiteThemeToggle />
    </div>
  );
}

/** Site-wide light/dark mode switch; defaults to light until a user toggles. */
export async function SiteThemeToggle() {
  const t = await getTranslations("common.theme");
  const labels: ThemeToggleLabels = {
    dark: t("dark"),
    light: t("light"),
    switchToDark: t("switchToDark"),
    switchToLight: t("switchToLight"),
  };

  return <ThemeToggle labels={labels} />;
}

/** Site-wide footer legal row. */
export async function FooterLegalRow() {
  const t = await getTranslations("common.footer.legal");
  return (
    <div className="flex flex-col items-center justify-center gap-3 text-center">
      <span>{t("copyright", { year: new Date().getFullYear() })}</span>
    </div>
  );
}
