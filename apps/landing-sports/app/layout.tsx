import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";
import "./globals.css";

// SPEC-021 — sports landing variant (variant.id "sports").
// Public marketing surface, so robots are indexable (unlike apps/liff).

export const metadata: Metadata = {
  title: "Prophetica — 運動賽事預測",
  description:
    "以站內虛擬銅幣對未公開的運動賽事結果進行預測，由中立撮合引擎與其他玩家公平成交。",
  applicationName: "Prophetica Sports",
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
};

export default function LandingSportsLayout({
  children,
}: {
  children: ReactNode;
}) {
  return (
    <html lang="zh-Hant">
      <body>{children}</body>
    </html>
  );
}
