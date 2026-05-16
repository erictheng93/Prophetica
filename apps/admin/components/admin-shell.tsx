import type { ReactNode } from "react";
import { adminCopy } from "@/lib/admin-copy";
import { getAdminOperationsSnapshot } from "@/lib/ops-facade";
import { canAccess, filterRoutes, type Role } from "@/lib/rbac";
import { AdminNavDrawer } from "./admin-nav-drawer";
import { AdminSidebar } from "./admin-nav";

interface AdminShellProps {
  children: ReactNode;
  eyebrow?: string;
  title: string;
  description: string;
  role: Role;
}

const roleLabel: Record<Role, string> = {
  admin: adminCopy.shell.roleAdmin,
  manager: adminCopy.shell.roleManager,
  affiliate: adminCopy.shell.roleAffiliate,
};

/*
 * Two-column shell:
 *   md+   — persistent sidebar (surface-black) on the left, parchment
 *           page header + white body in the main column on the right
 *   < md  — single column; a thin dark top strip carries the role chip
 *           and a hamburger that opens AdminNavDrawer
 *
 * Visual rhythm follows DESIGN.md §Components: surface-black for the
 * always-visible navigation surface (sidebar / mobile top strip), the
 * sub-nav-frosted parchment for the page header below it, and canvas
 * white for the body — mirroring Apple's global-nav → sub-nav → tile
 * stack on a per-page basis.
 */
export function AdminShell({
  children,
  eyebrow = adminCopy.shell.defaultEyebrow,
  title,
  description,
  role,
}: AdminShellProps) {
  const { routes } = getAdminOperationsSnapshot();
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
  const navLinks = [
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

  return (
    <div className="min-h-screen bg-proph-canvas text-proph-ink md:flex">
      <AdminSidebar
        ariaLabel={adminCopy.shell.navAria}
        links={navLinks}
        signOutLabel={adminCopy.shell.signOut}
        roleLabel={roleLabel[role]}
        brandKicker={adminCopy.shell.defaultEyebrow}
        brandLabel={adminCopy.shell.brandLabel}
      />

      <div className="flex min-w-0 flex-1 flex-col">
        <div className="border-b border-proph-hairline bg-proph-canvas-parchment text-proph-ink md:hidden">
          <div className="px-5 py-3">
            <AdminNavDrawer
              ariaLabel={adminCopy.shell.navAria}
              links={navLinks}
              signOutLabel={adminCopy.shell.signOut}
              roleLabel={roleLabel[role]}
            />
          </div>
        </div>

        <header className="border-b border-proph-hairline bg-proph-canvas-parchment">
          <div className="px-5 py-8 md:px-10 md:py-10">
            <p className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-primary">
              {eyebrow}
            </p>
            <h1 className="mt-3 max-w-4xl text-[28px] font-semibold leading-[1.14] tracking-[-0.28px] text-proph-ink sm:text-[34px] sm:leading-[1.1] sm:tracking-[-0.374px] md:text-[40px] md:leading-[1.07] lg:text-[44px]">
              {title}
            </h1>
            <p className="mt-3 max-w-3xl text-[15px] leading-[1.47] tracking-[-0.224px] text-proph-ink-muted-80 sm:text-[17px] sm:tracking-[-0.374px]">
              {description}
            </p>
          </div>
        </header>

        <div className="px-5 py-8 md:px-10 md:py-12">{children}</div>
      </div>
    </div>
  );
}
