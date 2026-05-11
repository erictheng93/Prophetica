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
 * Mobile-only navigation drawer (< md). Matches the new light AdminSidebar:
 * parchment surface, ink text, Action Blue for the active route. The
 * dark sidebar / drawer combo was replaced 2026-05-11 so the admin
 * surface reads as a single coherent palette rather than a dark nav
 * appended to a light dashboard.
 *
 * Affordances kept from the previous dark drawer:
 *   - Trigger 44x44 (Apple touch-target guideline)
 *   - Esc closes; route change closes; click outside the panel closes
 *   - role="dialog" + aria-modal on panel
 *   - Body scroll locked while open (iOS Safari rubber-band guard)
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
          className="inline-flex rounded-proph-pill border border-proph-hairline bg-proph-canvas px-3 py-1.5 text-[12px] font-semibold leading-none tracking-[-0.12px] text-proph-ink-muted-80"
        >
          {roleLabel}
        </span>
        <button
          type="button"
          onClick={() => setOpen(true)}
          aria-controls={panelId}
          aria-expanded={open}
          aria-label="開啟導覽選單"
          className="inline-flex h-11 w-11 items-center justify-center rounded-proph-sm border border-proph-hairline bg-proph-canvas text-proph-ink transition-colors hover:bg-proph-divider-soft active:bg-proph-canvas-parchment"
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
          className="fixed inset-0 z-50 flex flex-col bg-proph-canvas-parchment text-proph-ink md:hidden"
        >
          <div className="flex items-center justify-between border-b border-proph-hairline px-5 py-4">
            <span className="text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
              導覽
            </span>
            <button
              type="button"
              onClick={() => setOpen(false)}
              aria-label="關閉導覽選單"
              className="inline-flex h-11 w-11 items-center justify-center rounded-proph-sm border border-proph-hairline bg-proph-canvas text-proph-ink transition-colors hover:bg-proph-divider-soft active:bg-proph-canvas-parchment"
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
                          ? "bg-proph-canvas text-proph-primary shadow-[inset_0_0_0_1px_var(--proph-color-hairline)]"
                          : "text-proph-ink-muted-80 hover:bg-proph-canvas hover:text-proph-ink active:bg-proph-divider-soft",
                      )}
                    >
                      {link.label}
                    </Link>
                  </li>
                );
              })}
            </ul>
          </nav>
          <div className="border-t border-proph-hairline px-5 py-4">
            <button
              type="button"
              onClick={handleSignOut}
              className="flex h-12 w-full items-center justify-center rounded-proph-sm bg-proph-ink text-[15px] leading-[1.29] tracking-[-0.224px] text-proph-on-dark transition-opacity hover:opacity-90 active:opacity-80"
            >
              {signOutLabel}
            </button>
          </div>
        </div>
      )}
    </>
  );
}
