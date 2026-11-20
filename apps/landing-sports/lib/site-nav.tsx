import Link from "next/link";
import { getTranslations } from "next-intl/server";
import { ProphLinkButton } from "@prophetica/ui";
import { LanguageSwitcher } from "@/components/language-switcher";
import { ThemeToggle, type ThemeToggleLabels } from "@/components/theme-toggle";

const navItems = [
  { key: "markets", href: "/markets" },
  { key: "howItWorks", href: "/how-it-works" },
  { key: "responsibleGaming", href: "/responsible-gaming" },
] as const;

export async function SportsBrand() {
  const t = await getTranslations("common");
  return (
    <Link href="/" className="text-[#F0D78C] hover:opacity-80">
      {t("brand")}
    </Link>
  );
}

export async function SportsNavLinks() {
  const t = await getTranslations("common.nav");
  return (
    <>
      {navItems.map(({ key, href }) => (
        <a
          key={href}
          href={href}
          className="font-text text-[15px] leading-[1.35] tracking-[0] text-[var(--sports-muted)] transition-colors hover:text-[#F0D78C]"
        >
          {t(key)}
        </a>
      ))}
    </>
  );
}

export async function SportsAuthCta() {
  const t = await getTranslations("common.auth");
  return (
    <div className="flex items-center gap-2 sm:gap-3">
      <LanguageSwitcher />
      <nav
        aria-label={t("account.ariaLabel")}
        className="flex items-center gap-2"
      >
        <a
          href="/auth/sign-in"
          className="font-text text-[14px] leading-[1.29] tracking-[0] text-[var(--sports-muted)] transition-colors hover:text-[#F0D78C]"
        >
          {t("signIn")}
        </a>
        <ProphLinkButton
          variant="primary"
          href="/auth/sign-in?mode=signup"
          className="border-0 bg-[#D4A857] px-4 py-2 text-[14px] leading-[1.29] tracking-[0] text-[#050303]"
        >
          {t("signUp")}
        </ProphLinkButton>
      </nav>
      <SiteThemeToggle />
    </div>
  );
}

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

export async function getSportsFooterColumns() {
  const t = await getTranslations("common.footer");
  return [
    {
      heading: t("play.heading"),
      links: [
        { label: t("play.markets"), href: "/markets" },
        { label: t("play.howItWorks"), href: "/how-it-works" },
      ],
    },
    {
      heading: t("compliance.heading"),
      links: [
        {
          label: t("compliance.responsibleGaming"),
          href: "/responsible-gaming",
        },
        { label: t("compliance.terms"), href: "/legal/terms" },
        { label: t("compliance.privacy"), href: "/legal/privacy" },
      ],
    },
  ];
}

export async function SportsFooterLegalRow() {
  const t = await getTranslations("common.footer.legal");
  return <p>{t("body")}</p>;
}
