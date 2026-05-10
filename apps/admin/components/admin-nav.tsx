"use client";

import { useTransition } from "react";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { cn } from "@prophetica/ui";

interface AdminNavLink {
  href: string;
  label: string;
  key: string;
}

interface AdminNavProps {
  ariaLabel: string;
  links: AdminNavLink[];
  signOutLabel: string;
  roleLabel: string;
}

export function AdminNav({ ariaLabel, links, signOutLabel, roleLabel }: AdminNavProps) {
  const pathname = usePathname();
  const router = useRouter();
  const [pending, startTransition] = useTransition();

  async function handleSignOut() {
    await fetch("/api/auth/logout", { method: "POST" });
    startTransition(() => {
      router.replace("/login");
      router.refresh();
    });
  }

  return (
    <div className="hidden flex-wrap items-center justify-between gap-3 md:flex">
      <nav aria-label={ariaLabel} className="flex flex-wrap gap-2">
        {links.map((link) => {
          const isActive =
            link.href === "/" ? pathname === "/" : pathname === link.href || pathname?.startsWith(`${link.href}/`);

          return (
            <Link
              key={link.key}
              aria-current={isActive ? "page" : undefined}
              className={cn(
                "inline-flex min-h-11 items-center rounded-proph-sm px-3 py-2 text-[14px] leading-[1.29] tracking-[-0.224px] transition-colors",
                isActive
                  ? "bg-white/15 text-white"
                  : "text-proph-primary-on-dark hover:bg-white/10 active:bg-white/15",
              )}
              href={link.href}
            >
              {link.label}
            </Link>
          );
        })}
      </nav>
      <div className="flex items-center gap-3">
        <span
          aria-label="目前登入身份"
          className="rounded-proph-pill border border-white/20 px-3 py-1.5 text-[12px] font-semibold leading-none tracking-[-0.12px] text-proph-primary-on-dark"
        >
          {roleLabel}
        </span>
        <button
          type="button"
          onClick={handleSignOut}
          disabled={pending}
          className="inline-flex min-h-11 items-center rounded-proph-sm bg-white/10 px-4 py-2 text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-on-dark transition-colors hover:bg-white/20 active:bg-white/25 disabled:opacity-60"
        >
          {signOutLabel}
        </button>
      </div>
    </div>
  );
}
