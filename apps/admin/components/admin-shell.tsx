import type { ReactNode } from "react";
import { adminCopy } from "@/lib/admin-copy";
import { getAdminOperationsSnapshot } from "@/lib/ops-facade";
import { canAccess, filterRoutes, type Role } from "@/lib/rbac";
import { AdminNav } from "./admin-nav";
import { AdminNavDrawer } from "./admin-nav-drawer";

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
    baseLinks.push({ key: "overview", href: "/", label: adminCopy.shell.overview });
  }
  if (canAccess("/dashboard", role)) {
    baseLinks.push({ key: "dashboard", href: "/dashboard", label: adminCopy.shell.dashboard });
  }
  if (canAccess("/analytics", role)) {
    baseLinks.push({ key: "analytics", href: "/analytics", label: adminCopy.shell.analytics });
  }
  if (canAccess("/me/affiliate", role)) {
    baseLinks.push({ key: "me-affiliate", href: "/me/affiliate", label: adminCopy.affiliatePortal.title });
  }
  const navLinks = [
    ...baseLinks,
    ...visibleRoutes.map((route) => ({ key: route.domain, href: route.href, label: route.label })),
  ];

  return (
    <main className="min-h-screen bg-proph-canvas text-proph-ink">
      <header className="border-b border-proph-hairline bg-proph-surface-black text-proph-on-dark">
        <div className="mx-auto flex max-w-proph flex-col gap-5 px-5 py-6 md:gap-6 md:px-10 md:py-10">
          <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
            <div>
              <p className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-primary-on-dark">
                {eyebrow}
              </p>
              <h1 className="mt-3 max-w-4xl text-[28px] font-semibold leading-[1.14] tracking-[-0.28px] sm:text-[34px] sm:leading-[1.1] sm:tracking-[-0.374px] md:text-[44px] md:leading-[1.07]">
                {title}
              </h1>
              <p className="mt-3 max-w-3xl text-[15px] leading-[1.47] tracking-[-0.224px] text-proph-body-on-dark sm:text-[17px] sm:tracking-[-0.374px]">
                {description}
              </p>
            </div>
            <div className="hidden text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-body-on-dark md:block">
              {adminCopy.shell.readOnlyFacade}
              <br />
              {adminCopy.shell.noDirectLedgerWrites}
            </div>
          </div>
          <AdminNav
            ariaLabel={adminCopy.shell.navAria}
            links={navLinks}
            signOutLabel={adminCopy.shell.signOut}
            roleLabel={roleLabel[role]}
          />
          <AdminNavDrawer
            ariaLabel={adminCopy.shell.navAria}
            links={navLinks}
            signOutLabel={adminCopy.shell.signOut}
            roleLabel={roleLabel[role]}
          />
        </div>
      </header>
      <div className="mx-auto max-w-proph px-5 py-8 md:px-10 md:py-12">{children}</div>
    </main>
  );
}
