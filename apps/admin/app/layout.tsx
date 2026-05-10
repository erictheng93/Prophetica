import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";
import { ServiceWorkerRegistration } from "@/components/service-worker-registration";
import "./globals.css";

export const metadata: Metadata = {
  title: "Prophetica Admin",
  description: "Internal operations console. Access requires VPN + mTLS.",
  applicationName: "Prophetica Admin",
  appleWebApp: {
    capable: true,
    title: "Prophetica Admin",
    statusBarStyle: "black-translucent",
  },
  formatDetection: { telephone: false },
  robots: { index: false, follow: false },
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  maximumScale: 5,
  themeColor: "#1d1d1f",
  viewportFit: "cover",
};

export default function AdminLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="zh-Hant">
      <body>
        {children}
        <ServiceWorkerRegistration />
      </body>
    </html>
  );
}
