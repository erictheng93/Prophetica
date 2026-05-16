export const SUPPORTED_LOCALES = [
  "zh-Hant",
  "zh-Hans",
  "en",
  "id",
  "vi",
  "tl",
] as const;

export type SupportedLocale = (typeof SUPPORTED_LOCALES)[number];

export const DEFAULT_LOCALE: SupportedLocale = "zh-Hant";
export const LOCALE_COOKIE_NAME = "NEXT_LOCALE";
export const LOCALE_COOKIE_MAX_AGE_SECONDS = 60 * 60 * 24 * 365;

export const LOCALE_LABELS: Record<SupportedLocale, string> = {
  "zh-Hant": "繁體中文",
  "zh-Hans": "简体中文",
  en: "English",
  id: "Bahasa Indonesia",
  vi: "Tiếng Việt",
  tl: "Tagalog",
};

const localeAliases: Record<string, SupportedLocale> = {
  zh: "zh-Hant",
  "zh-hant": "zh-Hant",
  "zh-tw": "zh-Hant",
  "zh-hk": "zh-Hant",
  "zh-mo": "zh-Hant",
  "zh-hans": "zh-Hans",
  "zh-cn": "zh-Hans",
  "zh-sg": "zh-Hans",
  en: "en",
  "en-us": "en",
  id: "id",
  "id-id": "id",
  in: "id",
  vi: "vi",
  "vi-vn": "vi",
  tl: "tl",
  fil: "tl",
  "fil-ph": "tl",
};

export function isSupportedLocale(value: string): value is SupportedLocale {
  return SUPPORTED_LOCALES.includes(value as SupportedLocale);
}

export function resolveSupportedLocale(
  value: string | null | undefined,
): SupportedLocale | undefined {
  if (!value) return undefined;

  const normalized = value.trim().toLowerCase().replace(/_/g, "-");
  if (!normalized) return undefined;

  const exact = localeAliases[normalized];
  if (exact) return exact;

  const language = normalized.split("-")[0];
  return language ? localeAliases[language] : undefined;
}
