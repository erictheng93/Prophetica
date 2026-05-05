import type { ReactNode } from "react";
import "./globals.css";

export const metadata = {
  title: "prophetica",
  description: "SPEC-017 bootstrap app",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
