"use client";

import type { ReactNode } from "react";
import { usePathname } from "next/navigation";
import { isStandaloneAdminChromePath } from "@/lib/admin-chrome";
import { AdminNavDrawer } from "./admin-nav-drawer";
import { AdminSidebar } from "./admin-nav";

interface AdminNavLink {
  href: string;
  label: string;
  key: string;
  children?: AdminNavLink[];
}

interface AdminAppChromeProps {
  children: ReactNode;
  ariaLabel: string;
  brandKicker: string;
  brandLabel: string;
  links: AdminNavLink[];
  roleLabel: string | null;
  signOutLabel: string;
}

export function AdminAppChrome({
  children,
  ariaLabel,
  brandKicker,
  brandLabel,
  links,
  roleLabel,
  signOutLabel,
}: AdminAppChromeProps) {
  const pathname = usePathname();

  if (!roleLabel || isStandaloneAdminChromePath(pathname)) {
    return <>{children}</>;
  }

  return (
    <div className="min-h-screen bg-proph-canvas text-proph-ink md:flex">
      <AdminSidebar
        ariaLabel={ariaLabel}
        links={links}
        signOutLabel={signOutLabel}
        roleLabel={roleLabel}
        brandKicker={brandKicker}
        brandLabel={brandLabel}
      />

      <div className="flex min-w-0 flex-1 flex-col">
        <div className="border-b border-proph-hairline bg-proph-canvas-parchment text-proph-ink md:hidden">
          <div className="px-5 py-3">
            <AdminNavDrawer
              ariaLabel={ariaLabel}
              links={links}
              signOutLabel={signOutLabel}
              roleLabel={roleLabel}
            />
          </div>
        </div>

        {children}
      </div>
    </div>
  );
}
