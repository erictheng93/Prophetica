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
  const visibleRoutes = filterRoutes(routes, role).filter(
    (route) => !route.navHidden,
  );
  type NavChild = {
    key: string;
    href: string;
    label: string;
    badge?: string;
  };
  const baseLinks: (NavChild & { children?: NavChild[] })[] = [];

  if (canAccess("/", role)) {
    baseLinks.push({
      key: "overview",
      href: "/",
      label: adminCopy.shell.overview,
    });
  }
  // Decision support and analytics share the analytics-reporting backend,
  // so they collapse into one expandable「數據與報表」nav group rather than
  // two flat entries. See SPEC-017 §6.3.3. Both children carry the
  //「開發中」badge — same treatment as /approvals — until the feature is
  // signed off; drop the `badge` field here to remove it.
  const dataReportChildren: NavChild[] = [];
  if (canAccess("/dashboard", role)) {
    dataReportChildren.push({
      key: "dashboard",
      href: "/dashboard",
      label: adminCopy.shell.dashboard,
      badge: adminCopy.shell.inDevelopmentBadge,
    });
  }
  if (canAccess("/analytics", role)) {
    dataReportChildren.push({
      key: "analytics",
      href: "/analytics",
      label: adminCopy.shell.analytics,
      badge: adminCopy.shell.inDevelopmentBadge,
    });
  }
  const [firstDataReportChild] = dataReportChildren;
  if (firstDataReportChild) {
    baseLinks.push({
      key: "data-and-reports",
      // Parent is a non-navigable toggle; href only anchors active state.
      href: firstDataReportChild.href,
      label: adminCopy.shell.dataAndReports,
      children: dataReportChildren,
    });
  }
  if (canAccess("/security/login-events", role)) {
    baseLinks.push({
      key: "login-events",
      href: "/security/login-events",
      label: "登入事件",
    });
  }
  if (canAccess("/me/affiliate", role)) {
    baseLinks.push({
      key: "me-affiliate",
      href: "/me/affiliate",
      label: adminCopy.affiliatePortal.title,
    });
  }

  const inDevelopmentHrefs = new Set(
    routes.filter((route) => route.inDevelopment).map((route) => route.href),
  );

  // FAQ 管理 is content management, not an operational domain, so it sits
  // below the operational sections (會員管理 / 代理管理 / …) in the nav.
  // See SPEC-017 §6.3.3.
  const tailLinks: typeof baseLinks = [];
  if (canAccess("/faq", role)) {
    tailLinks.push({ key: "faq", href: "/faq", label: adminCopy.shell.faq });
  }

  return [
    ...baseLinks,
    ...visibleRoutes.map((route) => {
      const filteredChildren = route.children
        ?.filter((child) => canAccess(child.href, role))
        .map((child) =>
          inDevelopmentHrefs.has(child.href)
            ? { ...child, badge: adminCopy.shell.inDevelopmentBadge }
            : child,
        );
      return {
        key: route.domain,
        href: route.href,
        label: route.label,
        // Top-level routes (e.g. /compensation) carry the「開發中」badge
        // directly; children get it via inDevelopmentHrefs above.
        ...(route.inDevelopment
          ? { badge: adminCopy.shell.inDevelopmentBadge }
          : {}),
        ...(filteredChildren?.length ? { children: filteredChildren } : {}),
      };
    }),
    ...tailLinks,
  ];
}

export default async function AdminLayout({
  children,
}: {
  children: ReactNode;
}) {
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
