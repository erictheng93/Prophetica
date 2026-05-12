import { getRequestConfig } from "next-intl/server";
import { cookies } from "next/headers";
import {
  DEFAULT_LOCALE,
  LOCALE_COOKIE_NAME,
  resolveSupportedLocale,
} from "./locales";

type Messages = Record<string, unknown>;

function isRecord(value: unknown): value is Messages {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function mergeMessages(base: Messages, override: Messages): Messages {
  const merged: Messages = { ...base };

  for (const [key, value] of Object.entries(override)) {
    const baseValue = merged[key];
    merged[key] =
      isRecord(baseValue) && isRecord(value)
        ? mergeMessages(baseValue, value)
        : value;
  }

  return merged;
}

async function loadMessages(locale: string): Promise<Messages> {
  const defaultMessages = (await import(`../messages/${DEFAULT_LOCALE}.json`))
    .default as Messages;
  if (locale === DEFAULT_LOCALE) return defaultMessages;

  const localeMessages = (await import(`../messages/${locale}.json`))
    .default as Messages;
  return mergeMessages(defaultMessages, localeMessages);
}

export default getRequestConfig(async () => {
  const cookieStore = await cookies();
  const locale =
    resolveSupportedLocale(cookieStore.get(LOCALE_COOKIE_NAME)?.value) ??
    DEFAULT_LOCALE;
  const messages = await loadMessages(locale);

  return { locale, messages };
});
