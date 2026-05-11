import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";
import { NextIntlClientProvider } from "next-intl";
import { getLocale, getMessages } from "next-intl/server";
import { ServiceWorkerRegistration } from "@/components/service-worker-registration";
import "./globals.css";

export const metadata: Metadata = {
  title: {
    default: "Prophetica",
    template: "%s · Prophetica",
  },
  description: "Event-prediction experience built on Apple-style design.",
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
  const locale = await getLocale();
  const messages = await getMessages();
  return (
    <html lang={locale}>
      <body>
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
         * Copy stays hardcoded in zh-Hant rather than going through
         * next-intl because the fallback must work even when the runtime
         * locale resolver fails — and zh-Hant is the only locale ship
         * today (see apps/web/messages/).
         */}
        <noscript>
          <div className="fixed inset-0 z-50 flex items-center justify-center bg-proph-canvas-parchment px-6 py-12">
            <div className="w-full max-w-[480px] text-center">
              <p className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-feedback-warning">
                需要 JavaScript
              </p>
              <h1 className="mt-3 font-display text-[28px] font-semibold leading-[1.14] tracking-[-0.28px] text-proph-ink sm:text-[34px] sm:leading-[1.1] sm:tracking-[-0.374px] md:text-[40px] md:leading-[1.07]">
                請啟用 JavaScript 以使用 Prophetica
              </h1>
              <p className="mt-4 text-[15px] leading-[1.47] tracking-[-0.224px] text-proph-ink-muted-80 sm:text-[17px] sm:tracking-[-0.374px]">
                即時下單、KYC 驗證、行情與點數結算都需要
                JavaScript。請在瀏覽器設定中開啟 JavaScript，或改用支援
                JavaScript 的瀏覽器。
              </p>
              <a
                href="/support/contact"
                className="mt-8 inline-flex h-12 min-w-[160px] items-center justify-center rounded-proph-pill bg-proph-primary px-6 text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-on-primary"
              >
                聯絡客服
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
