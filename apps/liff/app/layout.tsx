import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";
import { ServiceWorkerRegistration } from "@/components/service-worker-registration";
import "./globals.css";

/*
 * LIFF runs inside a LINE webview that already constrains the viewport.
 * The explicit viewport prevents older LINE renderers from falling back to
 * desktop-width emulation.
 */

export const metadata: Metadata = {
  title: "Prophetica LIFF",
  description: "Prophetica LINE mini app with an installable browser fallback.",
  applicationName: "Prophetica LIFF",
  appleWebApp: {
    capable: true,
    title: "Prophetica LIFF",
    statusBarStyle: "default",
  },
  icons: {
    icon: [{ url: "/favicon-32.png", sizes: "32x32", type: "image/png" }],
    apple: [
      { url: "/apple-touch-icon.png", sizes: "180x180", type: "image/png" },
    ],
  },
  robots: { index: false, follow: false },
  formatDetection: { telephone: false },
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  maximumScale: 1,
  userScalable: false,
  viewportFit: "cover",
  themeColor: "#0066cc",
};

export default function LiffLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="zh-Hant">
      <body>
        {children}
        <ServiceWorkerRegistration />
      </body>
    </html>
  );
}
