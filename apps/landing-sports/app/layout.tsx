import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";
import { NextIntlClientProvider } from "next-intl";
import { getLocale, getMessages, getTranslations } from "next-intl/server";
import "./globals.css";

// SPEC-021 - sports landing variant (variant.id "sports").
// Public marketing surface, so robots are indexable (unlike apps/liff).

const themeInitScript = `
(() => {
  try {
    const stored = window.localStorage.getItem("prophetica-theme");
    const theme = stored === "light" ? "light" : "dark";
    const root = document.documentElement;
    root.dataset.theme = theme;
    root.style.colorScheme = theme;
  } catch {
    const root = document.documentElement;
    root.dataset.theme = "dark";
    root.style.colorScheme = "dark";
  }
})();
`;

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("common.metadata");

  return {
    title: t("title"),
    description: t("description"),
    applicationName: "Prophetica Sports",
  };
}

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
};

export default async function LandingSportsLayout({
  children,
}: {
  children: ReactNode;
}) {
  const [locale, messages, tNoScript] = await Promise.all([
    getLocale(),
    getMessages(),
    getTranslations("common.noScript"),
  ]);

  return (
    <html lang={locale} data-theme="dark" suppressHydrationWarning>
      <body>
        <script dangerouslySetInnerHTML={{ __html: themeInitScript }} />
        <noscript>
          <div className="fixed inset-0 z-50 flex items-center justify-center bg-[var(--sports-bg)] px-6 py-12">
            <div className="w-full max-w-[480px] text-center">
              <h1 className="font-display text-[34px] font-semibold leading-[1.1] text-[var(--sports-heading)]">
                {tNoScript("headline")}
              </h1>
              <p className="mt-4 font-text text-[17px] leading-[1.6] text-[var(--sports-muted)]">
                {tNoScript("body")}
              </p>
              <a
                href="/support/contact"
                className="mt-8 inline-flex h-12 min-w-[160px] items-center justify-center rounded-proph-pill bg-[#D4A857] px-6 text-[17px] leading-none text-[#050303]"
              >
                {tNoScript("cta")}
              </a>
            </div>
          </div>
        </noscript>
        <NextIntlClientProvider locale={locale} messages={messages}>
          {children}
        </NextIntlClientProvider>
      </body>
    </html>
  );
}
