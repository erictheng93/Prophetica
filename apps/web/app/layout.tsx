import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";
import { NextIntlClientProvider } from "next-intl";
import { getLocale, getMessages, getTranslations } from "next-intl/server";
import { ServiceWorkerRegistration } from "@/components/service-worker-registration";
import "./globals.css";

const themeInitScript = `
(() => {
  try {
    const stored = window.localStorage.getItem("prophetica-theme");
    const theme = stored === "dark" ? "dark" : "light";
    const root = document.documentElement;
    root.dataset.theme = theme;
    root.style.colorScheme = theme;
  } catch {
    const root = document.documentElement;
    root.dataset.theme = "light";
    root.style.colorScheme = "light";
  }
})();
`;

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("common.metadata");

  return {
    title: {
      default: "Prophetica",
      template: "%s · Prophetica",
    },
    description: t("description"),
    applicationName: "Prophetica",
    appleWebApp: {
      capable: true,
      title: "Prophetica",
      statusBarStyle: "default",
    },
    icons: {
      icon: [{ url: "/favicon-32.png", sizes: "32x32", type: "image/png" }],
      apple: [
        { url: "/apple-touch-icon.png", sizes: "180x180", type: "image/png" },
      ],
    },
    formatDetection: { telephone: false },
  };
}

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  maximumScale: 5,
  themeColor: "#0066cc",
  viewportFit: "cover",
};

export default async function RootLayout({
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
    <html lang={locale} data-theme="light" suppressHydrationWarning>
      <body>
        <script dangerouslySetInnerHTML={{ __html: themeInitScript }} />
        {/*
         * noscript fallback — every interactive surface on Prophetica
         * (trade panel, KYC submission, top-up flow, sign-in, real-time
         * prices) is JS-driven. When the browser cannot run JS the SSR
         * snapshot would render but every form would silently fail; we
         * cover the page with an explicit warning instead.
         *
         * The overlay is fixed + z-50 so it sits above any SSR'd content
         * even before hydration; when JS is on the browser elides the
         * <noscript> entirely and nothing renders. Tailwind utilities
         * resolve via the stylesheet in <head>, which loads regardless
         * of JS state, so the message inherits Apple design tokens.
         *
         * Copy is resolved on the server so users who disable JavaScript still
         * get the selected locale from the initial request.
         */}
        <noscript>
          <div className="fixed inset-0 z-50 flex items-center justify-center bg-proph-canvas-parchment px-6 py-12">
            <div className="w-full max-w-[480px] text-center">
              <p className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-feedback-warning">
                {tNoScript("kicker")}
              </p>
              <h1 className="mt-3 font-display text-[28px] font-semibold leading-[1.14] tracking-[-0.28px] text-proph-ink sm:text-[34px] sm:leading-[1.1] sm:tracking-[-0.374px] md:text-[40px] md:leading-[1.07]">
                {tNoScript("headline")}
              </h1>
              <p className="mt-4 text-[15px] leading-[1.47] tracking-[-0.224px] text-proph-ink-muted-80 sm:text-[17px] sm:tracking-[-0.374px]">
                {tNoScript("body")}
              </p>
              <a
                href="/support/contact"
                className="mt-8 inline-flex h-12 min-w-[160px] items-center justify-center rounded-proph-pill bg-proph-primary px-6 text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary"
              >
                {tNoScript("cta")}
              </a>
            </div>
          </div>
        </noscript>
        <NextIntlClientProvider locale={locale} messages={messages}>
          {children}
        </NextIntlClientProvider>
        <ServiceWorkerRegistration />
      </body>
    </html>
  );
}
