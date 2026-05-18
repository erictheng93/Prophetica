import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";
import { AdminAppChrome } from "@/components/admin-app-chrome";
import { ServiceWorkerRegistration } from "@/components/service-worker-registration";
import { adminCopy } from "@/lib/admin-copy";
import { getAdminOperationsRoutes } from "@/lib/ops-facade";
import { canAccess, filterRoutes, type Role } from "@/lib/rbac";
import { getViewer } from "@/lib/session";
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
  icons: {
    icon: [{ url: "/favicon-32.png", sizes: "32x32", type: "image/png" }],
    apple: [
      { url: "/apple-touch-icon.png", sizes: "180x180", type: "image/png" },
    ],
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

const roleLabel: Record<Role, string> = {
  admin: adminCopy.shell.roleAdmin,
  manager: adminCopy.shell.roleManager,
  affiliate: adminCopy.shell.roleAffiliate,
};

function buildAdminNavLinks(role: Role) {
  const routes = getAdminOperationsRoutes();
  const visibleRoutes = filterRoutes(routes, role);
  const baseLinks: { key: string; href: string; label: string }[] = [];

  if (canAccess("/", role)) {
    baseLinks.push({
      key: "overview",
      href: "/",
      label: adminCopy.shell.overview,
    });
  }
  if (canAccess("/dashboard", role)) {
    baseLinks.push({
      key: "dashboard",
      href: "/dashboard",
      label: adminCopy.shell.dashboard,
    });
  }
  if (canAccess("/analytics", role)) {
    baseLinks.push({
      key: "analytics",
      href: "/analytics",
      label: adminCopy.shell.analytics,
    });
  }
  if (canAccess("/security/login-events", role)) {
    baseLinks.push({
      key: "login-events",
      href: "/security/login-events",
      label: "登入事件",
    });
  }
  if (canAccess("/faq", role)) {
    baseLinks.push({ key: "faq", href: "/faq", label: adminCopy.shell.faq });
  }
  if (canAccess("/me/affiliate", role)) {
    baseLinks.push({
      key: "me-affiliate",
      href: "/me/affiliate",
      label: adminCopy.affiliatePortal.title,
    });
  }

  return [
    ...baseLinks,
    ...visibleRoutes.map((route) => {
      const filteredChildren = route.children?.filter((child) =>
        canAccess(child.href, role),
      );
      return {
        key: route.domain,
        href: route.href,
        label: route.label,
        ...(filteredChildren?.length ? { children: filteredChildren } : {}),
      };
    }),
  ];
}

export default async function AdminLayout({ children }: { children: ReactNode }) {
  const viewer = await getViewer();
  const links = viewer ? buildAdminNavLinks(viewer.role) : [];

  return (
    <html lang="zh-Hant">
      <body>
        <AdminAppChrome
          ariaLabel={adminCopy.shell.navAria}
          brandKicker={adminCopy.shell.defaultEyebrow}
          brandLabel={adminCopy.shell.brandLabel}
          links={links}
          roleLabel={viewer ? roleLabel[viewer.role] : null}
          signOutLabel={adminCopy.shell.signOut}
        >
          {children}
        </AdminAppChrome>
        <ServiceWorkerRegistration />
      </body>
    </html>
  );
}
