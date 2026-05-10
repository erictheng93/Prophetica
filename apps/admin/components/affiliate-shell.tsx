import type { ReactNode } from "react";
import Link from "next/link";
import { adminCopy } from "@/lib/admin-copy";
import { AdminNav } from "./admin-nav";

interface AffiliateShellProps {
  children: ReactNode;
  affiliateId: string;
  active: "home" | "payouts" | "sub-tree";
}

const subNav: { key: AffiliateShellProps["active"]; href: string; label: string }[] = [
  { key: "home", href: "/me/affiliate", label: "概覽" },
  { key: "payouts", href: "/me/affiliate/payouts", label: "撥款進度" },
  { key: "sub-tree", href: "/me/affiliate/sub-tree", label: "推薦鏈" },
];

export function AffiliateShell({ children, affiliateId, active }: AffiliateShellProps) {
  return (
    <main className="min-h-screen bg-proph-canvas text-proph-ink">
      <header className="border-b border-proph-hairline bg-proph-surface-black text-proph-on-dark">
        <div className="mx-auto flex max-w-proph flex-col gap-6 px-6 py-8 md:px-10 md:py-10">
          <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
            <div>
              <p className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-primary-on-dark">
                {adminCopy.affiliatePortal.eyebrow}
              </p>
              <h1 className="mt-3 max-w-4xl text-[34px] font-semibold leading-[1.1] tracking-[-0.374px] md:text-[44px] md:leading-[1.07] md:tracking-[-0.28px]">
                {adminCopy.affiliatePortal.title}
              </h1>
              <p className="mt-4 max-w-3xl text-[17px] leading-[1.47] tracking-[-0.374px] text-proph-body-on-dark">
                {adminCopy.affiliatePortal.description}
              </p>
            </div>
            <div className="text-[14px] leading-[1.43] tracking-[-0.224px] text-proph-body-on-dark">
              <p className="text-proph-primary-on-dark">{adminCopy.affiliatePortal.affiliateIdLabel}</p>
              <p className="font-mono text-[17px] tracking-[-0.374px]">{affiliateId}</p>
            </div>
          </div>
          <AdminNav
            ariaLabel={adminCopy.shell.navAria}
            links={subNav.map((item) => ({ key: item.key, href: item.href, label: item.label }))}
            signOutLabel={adminCopy.shell.signOut}
            roleLabel={adminCopy.shell.roleAffiliate}
          />
        </div>
      </header>
      <div className="mx-auto max-w-proph px-6 py-10 md:px-10 md:py-12">{children}</div>
      <p hidden data-active-section={active} />
      <noscript>
        <Link href="/login">登入</Link>
      </noscript>
    </main>
  );
}
