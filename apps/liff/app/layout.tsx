import type { ReactNode } from "react";
import "./globals.css";

export const metadata = {
  title: "prophetica · LIFF",
  description: "prophetica LINE mini app — quick access from inside LINE.",
  robots: { index: false, follow: false },
};

export default function LiffLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="zh-Hant">
      <body>{children}</body>
    </html>
  );
}
