import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";
import "./globals.css";

/*
 * SPEC-017 §6.3 LIFF — runs inside a LINE webview that already constrains
 * the viewport to a narrow column. We still ship an explicit viewport
 * declaration so the in-LINE renderer cannot fall back to its default
 * 980px desktop emulation when LINE versions diverge across iOS / Android.
 *
 * `userScalable: false` is intentional for LIFF: LINE itself owns the
 * gesture stack (swipe-to-dismiss the mini-window). Letting users pinch
 * inside the embed creates layout glitches against LINE's parent chrome.
 */

export const metadata: Metadata = {
  title: "prophetica · LIFF",
  description: "prophetica LINE mini app — quick access from inside LINE.",
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
      <body>{children}</body>
    </html>
  );
}
