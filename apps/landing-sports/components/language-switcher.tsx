"use client";

import { useRouter } from "next/navigation";
import { useLocale, useTranslations } from "next-intl";
import {
  DEFAULT_LOCALE,
  LOCALE_COOKIE_MAX_AGE_SECONDS,
  LOCALE_COOKIE_NAME,
  LOCALE_LABELS,
  SUPPORTED_LOCALES,
  isSupportedLocale,
  type SupportedLocale,
} from "@/i18n/locales";

export function LanguageSwitcher() {
  const locale = useLocale();
  const router = useRouter();
  const t = useTranslations("common.localeSwitcher");
  const selectedLocale = isSupportedLocale(locale) ? locale : DEFAULT_LOCALE;

  function switchLocale(nextLocale: SupportedLocale) {
    document.cookie = [
      `${LOCALE_COOKIE_NAME}=${nextLocale}`,
      `Max-Age=${LOCALE_COOKIE_MAX_AGE_SECONDS}`,
      "Path=/",
      "SameSite=Lax",
    ].join("; ");
    router.refresh();
  }

  return (
    <label className="inline-flex min-h-11 items-center">
      <span className="sr-only">{t("label")}</span>
      <select
        aria-label={t("label")}
        value={selectedLocale}
        onChange={(event) =>
          switchLocale(event.target.value as SupportedLocale)
        }
        className="h-9 max-w-[132px] rounded-proph-pill border border-[#D4A857]/35 bg-[var(--sports-control-bg)] px-3 pr-8 font-text text-[13px] leading-none tracking-[0] text-[var(--sports-text)] transition-colors hover:border-[#D4A857] focus:border-[#F0D78C] focus:outline-none sm:max-w-none"
      >
        {SUPPORTED_LOCALES.map((option) => (
          <option key={option} value={option}>
            {LOCALE_LABELS[option]}
          </option>
        ))}
      </select>
    </label>
  );
}
