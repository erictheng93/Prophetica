"use client";

import { useState, useTransition } from "react";
import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { cn } from "@prophetica/ui";
import { findActiveNavChildKey, isNavPathActive } from "@/lib/admin-nav-active";

interface AdminNavLink {
  href: string;
  label: string;
  key: string;
  children?: AdminNavLink[];
  badge?: string;
}

interface AdminNavProps {
  ariaLabel: string;
  links: AdminNavLink[];
  signOutLabel: string;
  roleLabel: string;
}

/*
 * Horizontal nav row (legacy). Still used by affiliate-shell as the
 * sub-nav strip for the affiliate portal (概覽 / 撥款進度 / 推薦鏈).
 * The admin shell now uses <AdminSidebar> (below) for its primary
 * navigation; do not add it back to admin-shell.tsx.
 */
export function AdminNav({
  ariaLabel,
  links,
  signOutLabel,
  roleLabel,
}: AdminNavProps) {
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
    <div className="hidden flex-col gap-3 md:flex lg:flex-row lg:flex-wrap lg:items-center lg:justify-between">
      <nav aria-label={ariaLabel} className="flex flex-wrap gap-2 lg:order-1">
        {links.map((link) => {
          const isActive =
            link.href === "/"
              ? pathname === "/"
              : isNavPathActive(pathname, link.href);

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
      <div className="flex items-center gap-3 lg:order-2">
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

interface AdminSidebarProps {
  ariaLabel: string;
  links: AdminNavLink[];
  signOutLabel: string;
  roleLabel: string;
  brandLabel: string;
  brandKicker: string;
}

/*
 * Desktop sidebar (md+) for the admin app.
 *
 * Design — DESIGN.md §Components.sub-nav-frosted: canvas-parchment
 * background, hairline borders, ink text, the single Action Blue
 * (#0066cc) carries every interactive accent. The rail matches the rest
 * of the admin surface instead of reading as a separate "dark navigation
 * panel". Active link sits on a white card (canvas) with ink-on-blue
 * label and a 1px inset hairline that gives the chip a quiet lift.
 *
 * Mobile (< md) is handled by <AdminNavDrawer>. This component is
 * unconditionally hidden below md so the layout collapses to a single
 * column without it.
 */
export function AdminSidebar({
  ariaLabel,
  links,
  signOutLabel,
  roleLabel,
  brandLabel,
  brandKicker,
}: AdminSidebarProps) {
  const pathname = usePathname();
  const router = useRouter();
  const [pending, startTransition] = useTransition();
  const [expandedSections, setExpandedSections] = useState<
    Record<string, boolean>
  >({});

  async function handleSignOut() {
    await fetch("/api/auth/logout", { method: "POST" });
    startTransition(() => {
      router.replace("/login");
      router.refresh();
    });
  }

  function isSectionOpen(link: AdminNavLink): boolean {
    if (!link.children?.length) {
      return false;
    }

    // An explicit click wins: once the user toggles a section, that
    // choice is authoritative even while a child route is active.
    const explicit = expandedSections[link.key];
    if (explicit !== undefined) {
      return explicit;
    }

    // Untouched: auto-expand when a child route is currently active.
    return findActiveNavChildKey(pathname, link.children) !== undefined;
  }

  function toggleSection(link: AdminNavLink) {
    if (!link.children?.length) {
      return;
    }

    setExpandedSections((previous) => ({
      ...previous,
      [link.key]: !isSectionOpen(link),
    }));
  }

  return (
    <aside className="hidden md:sticky md:top-0 md:flex md:h-screen md:w-60 md:flex-col md:border-r md:border-proph-hairline md:bg-proph-canvas-parchment md:text-proph-ink lg:w-64">
      <div className="border-b border-proph-hairline px-6 py-6">
        <p className="text-[11px] font-semibold uppercase tracking-[0.08em] text-proph-ink-muted-48">
          {brandKicker}
        </p>
        <p className="mt-1 text-[17px] font-semibold leading-[1.24] tracking-[-0.374px] text-proph-ink">
          {brandLabel}
        </p>
      </div>

      <nav aria-label={ariaLabel} className="flex-1 overflow-y-auto px-3 py-4">
        <ul className="flex flex-col gap-1">
          {links.map((link) => {
            const activeChildKey = link.children
              ? findActiveNavChildKey(pathname, link.children)
              : undefined;
            const isExactActive = pathname === link.href;
            const isActive =
              isNavPathActive(pathname, link.href) ||
              activeChildKey !== undefined;
            const sectionOpen = isSectionOpen(link);
            return (
              <li key={link.key}>
                {link.children?.length ? (
                  <>
                    <button
                      type="button"
                      onClick={() => toggleSection(link)}
                      aria-expanded={sectionOpen}
                      aria-controls={`${link.key}-children`}
                      className={cn(
                        "flex min-h-11 w-full items-center justify-between rounded-proph-sm px-3 py-2 text-[14px] leading-[1.29] tracking-[-0.224px] transition-colors",
                        isActive
                          ? "bg-proph-canvas text-proph-primary shadow-[inset_0_0_0_1px_var(--proph-color-hairline)]"
                          : "text-proph-ink-muted-80 hover:bg-proph-canvas hover:text-proph-ink active:bg-proph-divider-soft",
                      )}
                    >
                      <span>{link.label}</span>
                      <span
                        aria-hidden="true"
                        className={cn(
                          "inline-block text-[11px] transition-transform duration-150 ease-out",
                          sectionOpen && "rotate-90",
                        )}
                      >
                        {">"}
                      </span>
                    </button>
                    {sectionOpen && (
                      <ul
                        id={`${link.key}-children`}
                        aria-label={`${link.label} 子導覽`}
                        className="mt-1 flex flex-col gap-1 border-l border-proph-hairline pl-3"
                      >
                        {link.children.map((child) => {
                          const isChildActive = activeChildKey === child.key;

                          return (
                            <li key={child.key}>
                              <Link
                                href={child.href}
                                aria-current={
                                  isChildActive ? "page" : undefined
                                }
                                className={cn(
                                  "flex min-h-9 items-center rounded-proph-sm px-3 py-2 text-[13px] leading-[1.31] tracking-[-0.12px] transition-colors",
                                  isChildActive
                                    ? "bg-proph-canvas text-proph-primary shadow-[inset_0_0_0_1px_var(--proph-color-hairline)]"
                                    : "text-proph-ink-muted-80 hover:bg-proph-canvas hover:text-proph-ink active:bg-proph-divider-soft",
                                )}
                              >
                                <span>{child.label}</span>
                                {child.badge ? (
                                  <span className="ml-2 inline-flex items-center rounded-proph-pill bg-proph-divider-soft px-1.5 py-0.5 text-[10px] font-semibold leading-none tracking-[-0.12px] text-proph-ink-muted-80">
                                    {child.badge}
                                  </span>
                                ) : null}
                              </Link>
                            </li>
                          );
                        })}
                      </ul>
                    )}
                  </>
                ) : (
                  <Link
                    href={link.href}
                    aria-current={
                      isExactActive && !link.children?.length
                        ? "page"
                        : undefined
                    }
                    className={cn(
                      "flex min-h-11 items-center rounded-proph-sm px-3 py-2 text-[14px] leading-[1.29] tracking-[-0.224px] transition-colors",
                      isActive
                        ? "bg-proph-canvas text-proph-primary shadow-[inset_0_0_0_1px_var(--proph-color-hairline)]"
                        : "text-proph-ink-muted-80 hover:bg-proph-canvas hover:text-proph-ink active:bg-proph-divider-soft",
                    )}
                  >
                    {link.label}
                  </Link>
                )}
              </li>
            );
          })}
        </ul>
      </nav>

      <div className="space-y-3 border-t border-proph-hairline px-5 py-4">
        <span
          aria-label="目前登入身份"
          className="inline-flex rounded-proph-pill border border-proph-hairline bg-proph-canvas px-3 py-1.5 text-[12px] font-semibold leading-none tracking-[-0.12px] text-proph-ink-muted-80"
        >
          {roleLabel}
        </span>
        <button
          type="button"
          onClick={handleSignOut}
          disabled={pending}
          className="flex h-11 w-full items-center justify-center rounded-proph-sm bg-proph-ink text-[14px] leading-[1.29] tracking-[-0.224px] text-proph-on-dark transition-opacity hover:opacity-90 active:opacity-80 disabled:opacity-50"
        >
          {signOutLabel}
        </button>
      </div>
    </aside>
  );
}
