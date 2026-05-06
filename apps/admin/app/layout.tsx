import type { ReactNode } from "react";
import "./globals.css";

export const metadata = {
  title: "prophetica admin — restricted",
  description: "Internal operations console. Access requires VPN + mTLS.",
  robots: { index: false, follow: false },
};

export default function AdminLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="zh-Hant">
      <body>{children}</body>
    </html>
  );
}
