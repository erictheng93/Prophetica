import type { ReactNode } from "react";
import { adminCopy } from "@/lib/admin-copy";

interface AdminShellProps {
  children: ReactNode;
  eyebrow?: string;
  title: string;
  description: string;
}

// Page shell only: renders the per-page header and body. Persistent navigation
// chrome lives in app/layout.tsx so route changes do not reset nav UI state.
export function AdminShell({
  children,
  eyebrow = adminCopy.shell.defaultEyebrow,
  title,
  description,
}: AdminShellProps) {
  return (
    <>
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
    </>
  );
}
