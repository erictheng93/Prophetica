import { getRequestConfig } from "next-intl/server";

// Single-locale bootstrap: zh-Hant only. SPEC-012 §8.3 lists 繁中 / 簡中 /
// 英文 / 日文 as first-launch targets — adding them is "drop a JSON file
// next to messages/zh-Hant.json + flip the locale resolver here."
//
// We deliberately skip a `[locale]` route segment for now to keep the URL
// space stable while the translation catalogue stabilises. Routing-level
// locale negotiation is a separate Tech Lead task once en/ja/ko stub
// catalogues are seeded.
export const SUPPORTED_LOCALES = ["zh-Hant"] as const;
export const DEFAULT_LOCALE: (typeof SUPPORTED_LOCALES)[number] = "zh-Hant";

export default getRequestConfig(async () => {
  const locale = DEFAULT_LOCALE;
  const messages = (await import(`../messages/${locale}.json`)).default;
  return { locale, messages };
});
