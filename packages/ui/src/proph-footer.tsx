import type { ReactNode } from "react";
import { cn } from "./cn";

export interface ProphFooterColumn {
  heading: string;
  links: { label: string; href: string }[];
}

export interface ProphFooterProps {
  columns: ProphFooterColumn[];
  /** Fine-print legal copy rendered at the very bottom in 12px ink-muted-48. */
  legal?: ReactNode;
  className?: string;
}

/*
 * DESIGN.md §"Footer" — canvas-parchment, dense link columns.
 * - vertical padding 64px
 * - column headings: caption-strong (14px / 600)
 * - link rows: dense-link (17px / 400 / 2.41 line-height — the relaxed leading IS the design)
 * - legal row: fine-print (12px) in ink-muted-48
 */
export function ProphFooter({ columns, legal, className }: ProphFooterProps) {
  return (
    <footer
      className={cn(
        "w-full bg-proph-canvas-parchment text-center text-proph-ink-muted-80",
        "py-16",
        className,
      )}
    >
      <div className="mx-auto max-w-[1440px] px-6 md:px-10">
        <div className="flex flex-wrap justify-center gap-10">
          {columns.map((col) => (
            <div
              key={col.heading}
              className="w-[calc((100%-2.5rem)/2)] max-w-[220px] md:w-[calc((100%-7.5rem)/4)] lg:w-[calc((100%-10rem)/5)]"
            >
              <h3 className="mb-3 font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] text-proph-ink">
                {col.heading}
              </h3>
              <ul>
                {col.links.map((link) => (
                  <li
                    key={link.href}
                    className="font-text text-[17px] font-normal leading-[2.41] tracking-[0]"
                  >
                    <a
                      href={link.href}
                      className="text-proph-ink-muted-80 transition-colors hover:text-proph-ink"
                    >
                      {link.label}
                    </a>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>

        {legal ? (
          <div className="mt-12 border-t border-proph-divider-soft pt-6 font-text text-[12px] leading-none tracking-[-0.12px] text-proph-ink-muted-80">
            {legal}
          </div>
        ) : null}
      </div>
    </footer>
  );
}
