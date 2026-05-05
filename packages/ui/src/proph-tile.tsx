import { forwardRef, type HTMLAttributes, type ReactNode } from "react";
import { cn } from "./cn";

export type ProphTileSurface = "light" | "parchment" | "dark" | "dark-2" | "dark-3";

export interface ProphTileProps extends HTMLAttributes<HTMLElement> {
  surface?: ProphTileSurface;
  /** Optional eyebrow/kicker rendered above the headline at caption-strong size. */
  kicker?: ReactNode;
  headline: ReactNode;
  /** One-line tagline at typography.lead (28px / 400). */
  tagline?: ReactNode;
  /** Pair of CTAs — typically two ProphButtons rendered as a horizontal row. */
  actions?: ReactNode;
  /** Product render / hero media — the only legal site for var(--proph-shadow-product). */
  media?: ReactNode;
  /** Reverse stack so media sits above the text block. Defaults to text-first. */
  mediaFirst?: boolean;
}

/*
 * DESIGN.md §"Cards & Containers" — full-bleed product-tile.
 * - rounded.none (tiles touch edges; color change is the divider)
 * - vertical padding spacing.section (80px)
 * - centered stack: kicker → headline → tagline → actions → media
 * - text-on-dark variant uses primary-on-dark for inline links (Sky Link Blue)
 */
const surfaceClasses: Record<ProphTileSurface, string> = {
  light: "bg-proph-canvas text-proph-ink",
  parchment: "bg-proph-canvas-parchment text-proph-ink",
  dark: "bg-proph-surface-tile-1 text-proph-on-dark",
  "dark-2": "bg-proph-surface-tile-2 text-proph-on-dark",
  "dark-3": "bg-proph-surface-tile-3 text-proph-on-dark",
};

export const ProphTile = forwardRef<HTMLElement, ProphTileProps>(
  ({ surface = "light", kicker, headline, tagline, actions, media, mediaFirst = false, className, ...props }, ref) => {
    const onDark = surface === "dark" || surface === "dark-2" || surface === "dark-3";
    const taglineColor = onDark ? "text-proph-body-muted" : "text-proph-ink-muted-80";

    const text = (
      <div className="flex flex-col items-center text-center">
        {kicker ? (
          <p className="mb-3 font-text text-[14px] font-semibold leading-[1.29] tracking-[-0.224px] uppercase">
            {kicker}
          </p>
        ) : null}
        <h2 className="font-display text-[40px] font-semibold leading-[1.1] tracking-[0]">{headline}</h2>
        {tagline ? (
          <p
            className={cn(
              "mt-4 font-display text-[28px] font-normal leading-[1.14] tracking-[0.196px]",
              taglineColor,
            )}
          >
            {tagline}
          </p>
        ) : null}
        {actions ? <div className="mt-7 flex flex-wrap items-center justify-center gap-4">{actions}</div> : null}
      </div>
    );

    const mediaBlock = media ? (
      <div className="mt-12 flex justify-center">
        <div className="proph-product-shadow">{media}</div>
      </div>
    ) : null;

    return (
      <section
        ref={ref}
        className={cn("w-full", "px-6 md:px-10", "py-[80px] md:py-[80px]", surfaceClasses[surface], className)}
        {...props}
      >
        <div className="mx-auto max-w-[1440px]">
          {mediaFirst ? (
            <>
              {mediaBlock}
              {text}
            </>
          ) : (
            <>
              {text}
              {mediaBlock}
            </>
          )}
        </div>
      </section>
    );
  },
);
ProphTile.displayName = "ProphTile";
