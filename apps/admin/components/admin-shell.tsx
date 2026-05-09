import type { ReactNode } from "react";
import Link from "next/link";
import { getAdminOperationsSnapshot } from "@/lib/ops-facade";

interface AdminShellProps {
  children: ReactNode;
  eyebrow?: string;
  title: string;
  description: string;
}

export function AdminShell({ children, eyebrow = "Internal operations", title, description }: AdminShellProps) {
  const { routes } = getAdminOperationsSnapshot();

  return (
    <main className="min-h-screen bg-proph-canvas text-proph-ink">
      <header className="border-b border-proph-hairline bg-proph-surface-black text-proph-on-dark">
        <div className="mx-auto flex max-w-proph flex-col gap-6 px-6 py-6 md:px-10">
          <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
            <div>
              <p className="text-[12px] font-semibold uppercase tracking-[0.08em] text-proph-primary-on-dark">
                {eyebrow}
              </p>
              <h1 className="mt-2 max-w-4xl text-[32px] font-semibold leading-[1.1] md:text-[44px]">{title}</h1>
              <p className="mt-3 max-w-3xl text-[15px] leading-6 text-proph-body-on-dark">{description}</p>
            </div>
            <div className="text-[13px] leading-5 text-proph-body-on-dark">
              Read-only facade
              <br />
              No direct ledger writes
            </div>
          </div>
          <nav aria-label="Admin operations" className="flex flex-wrap gap-2">
            <Link className="rounded-proph-sm px-3 py-2 text-[14px] text-proph-primary-on-dark hover:bg-white/10" href="/">
              Overview
            </Link>
            <Link
              className="rounded-proph-sm px-3 py-2 text-[14px] text-proph-primary-on-dark hover:bg-white/10"
              href="/dashboard"
            >
              Dashboard
            </Link>
            {routes.map((route) => (
              <Link
                className="rounded-proph-sm px-3 py-2 text-[14px] text-proph-primary-on-dark hover:bg-white/10"
                href={route.href}
                key={route.domain}
              >
                {route.label}
              </Link>
            ))}
          </nav>
        </div>
      </header>
      <div className="mx-auto max-w-proph px-6 py-8 md:px-10">{children}</div>
    </main>
  );
}
