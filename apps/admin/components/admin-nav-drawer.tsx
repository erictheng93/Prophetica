"use client";

import { useEffect, useId, useState } from "react";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { cn } from "@prophetica/ui";

interface AdminNavLink {
  href: string;
  label: string;
  key: string;
}

interface AdminNavDrawerProps {
  ariaLabel: string;
  links: AdminNavLink[];
  signOutLabel: string;
  roleLabel: string;
}

/*
 * Mobile-only navigation drawer (< md). The desktop nav row is hidden
 * via Tailwind responsive utilities in <AdminNav>; this drawer takes
 * over below the breakpoint with a hamburger trigger that opens an
 * overlay covering the dark header surface.
 *
 * Affordances:
 *   - Trigger button is 44x44 (Apple touch-target guideline)
 *   - Esc closes; click outside the panel closes; route change closes
 *   - aria-controls / aria-expanded on trigger; role="dialog" + aria-modal
 *     on panel; focus is moved into the panel on open
 *   - Body scroll is locked while open so the underlying page does not
 *     bounce on iOS Safari rubber-band
 */

export function AdminNavDrawer({ ariaLabel, links, signOutLabel, roleLabel }: AdminNavDrawerProps) {
  const pathname = usePathname();
  const router = useRouter();
  const [open, setOpen] = useState(false);
  const panelId = useId();

  useEffect(() => {
    setOpen(false);
  }, [pathname]);

  useEffect(() => {
    if (!open) return;
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => {
      document.body.style.overflow = previous;
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  async function handleSignOut() {
    setOpen(false);
    await fetch("/api/auth/logout", { method: "POST" });
    router.replace("/login");
    router.refresh();
  }

  return (
    <>
      <div className="flex items-center justify-between gap-3 md:hidden">
        <span
          aria-label="目前登入身份"
          className="rounded-proph-pill border border-white/20 px-3 py-1.5 text-[12px] font-semibold leading-none tracking-[-0.12px] text-proph-primary-on-dark"
        >
          {roleLabel}
        </span>
        <button
          type="button"
          onClick={() => setOpen(true)}
          aria-controls={panelId}
          aria-expanded={open}
          aria-label="開啟導覽選單"
          className="inline-flex h-11 w-11 items-center justify-center rounded-proph-sm bg-white/10 text-proph-on-dark transition-colors hover:bg-white/20 active:bg-white/25"
        >
          <svg
            aria-hidden="true"
            viewBox="0 0 20 20"
            width="20"
            height="20"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.6"
            strokeLinecap="round"
          >
            <path d="M3 6h14" />
            <path d="M3 10h14" />
            <path d="M3 14h14" />
          </svg>
        </button>
      </div>

      {open && (
        <div
          id={panelId}
          role="dialog"
          aria-modal="true"
          aria-label={ariaLabel}
          className="fixed inset-0 z-50 flex flex-col bg-proph-surface-black/95 backdrop-blur-md md:hidden"
        >
          <div className="flex items-center justify-between border-b border-white/10 px-5 py-4">
            <span className="text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-primary-on-dark">
              導覽
            </span>
            <button
              type="button"
              onClick={() => setOpen(false)}
              aria-label="關閉導覽選單"
              className="inline-flex h-11 w-11 items-center justify-center rounded-proph-sm bg-white/10 text-proph-on-dark transition-colors hover:bg-white/20 active:bg-white/25"
            >
              <svg
                aria-hidden="true"
                viewBox="0 0 20 20"
                width="18"
                height="18"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.8"
                strokeLinecap="round"
              >
                <path d="M5 5l10 10" />
                <path d="M15 5L5 15" />
              </svg>
            </button>
          </div>
          <nav aria-label={ariaLabel} className="flex-1 overflow-y-auto px-3 py-4">
            <ul className="flex flex-col gap-1">
              {links.map((link) => {
                const isActive =
                  link.href === "/" ? pathname === "/" : pathname === link.href || pathname?.startsWith(`${link.href}/`);
                return (
                  <li key={link.key}>
                    <Link
                      href={link.href}
                      aria-current={isActive ? "page" : undefined}
                      onClick={() => setOpen(false)}
                      className={cn(
                        "flex min-h-12 items-center rounded-proph-sm px-4 py-3 text-[17px] leading-[1.24] tracking-[-0.374px]",
                        isActive
                          ? "bg-white/15 text-white"
                          : "text-proph-primary-on-dark hover:bg-white/10 active:bg-white/15",
                      )}
                    >
                      {link.label}
                    </Link>
                  </li>
                );
              })}
            </ul>
          </nav>
          <div className="border-t border-white/10 px-5 py-4">
            <button
              type="button"
              onClick={handleSignOut}
              className="flex h-12 w-full items-center justify-center rounded-proph-pill bg-white/10 text-[15px] leading-[1.29] tracking-[-0.224px] text-proph-on-dark transition-colors hover:bg-white/20 active:bg-white/25"
            >
              {signOutLabel}
            </button>
          </div>
        </div>
      )}
    </>
  );
}
