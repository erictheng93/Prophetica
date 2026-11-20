import type { ReactNode } from "react";
import Link from "next/link";
import { adminCopy } from "@/lib/admin-copy";
import { AdminNav } from "./admin-nav";
import { AdminNavDrawer } from "./admin-nav-drawer";

interface AffiliateShellProps {
  children: ReactNode;
  affiliateId: string;
}

const subNav = [
  { key: "home", href: "/me/affiliate", label: "概覽" },
  { key: "points", href: "/me/affiliate/points", label: "會員點數" },
  { key: "payouts", href: "/me/affiliate/payouts", label: "撥款進度" },
  { key: "sub-tree", href: "/me/affiliate/sub-tree", label: "推薦鏈" },
] as const;

export function AffiliateShell({ children, affiliateId }: AffiliateShellProps) {
  return (
    <main className="min-h-screen bg-proph-canvas text-proph-ink">
      <header className="relative overflow-hidden border-b border-proph-hairline/80 bg-proph-surface-black text-proph-on-dark">
        <div
          aria-hidden="true"
          className="pointer-events-none absolute -right-24 -top-24 h-96 w-96 rounded-full bg-proph-gold-primary/10 blur-3xl"
        />
        <div className="relative mx-auto flex max-w-proph flex-col gap-5 px-5 py-6 md:gap-6 md:px-10 md:py-10">
          <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
            <div>
              <p className="font-text text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-primary-on-dark">
                {adminCopy.affiliatePortal.eyebrow}
              </p>
              <h1 className="mt-3 max-w-4xl font-display text-[28px] font-semibold leading-[1.14] tracking-[-0.28px] sm:text-[34px] sm:leading-[1.1] sm:tracking-[-0.374px] md:text-[44px] md:leading-[1.07]">
                {adminCopy.affiliatePortal.title}
              </h1>
              <p className="mt-3 max-w-3xl font-text text-[15px] leading-[1.47] tracking-[-0.224px] text-proph-body-on-dark sm:text-[17px] sm:tracking-[-0.374px]">
                {adminCopy.affiliatePortal.description}
              </p>
            </div>
            <div className="font-text text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-body-on-dark">
              <p className="text-proph-primary-on-dark">
                {adminCopy.affiliatePortal.affiliateIdLabel}
              </p>
              <p className="font-mono text-[17px] tracking-[-0.374px] text-white">
                {affiliateId}
              </p>
            </div>
          </div>
          <AdminNav
            ariaLabel={adminCopy.shell.navAria}
            links={subNav.map((item) => ({
              key: item.key,
              href: item.href,
              label: item.label,
            }))}
            signOutLabel={adminCopy.shell.signOut}
            roleLabel={adminCopy.shell.roleAffiliate}
          />
          <AdminNavDrawer
            ariaLabel={adminCopy.shell.navAria}
            links={subNav.map((item) => ({
              key: item.key,
              href: item.href,
              label: item.label,
            }))}
            signOutLabel={adminCopy.shell.signOut}
            roleLabel={adminCopy.shell.roleAffiliate}
          />
        </div>
      </header>
      <div className="mx-auto max-w-proph px-5 py-8 md:px-10 md:py-12">
        {children}
      </div>
      <noscript>
        <Link href="/login">登入</Link>
      </noscript>
    </main>
  );
}
