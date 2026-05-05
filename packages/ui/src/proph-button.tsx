import { forwardRef, type AnchorHTMLAttributes, type ButtonHTMLAttributes, type ReactNode } from "react";
import { cn } from "./cn";

export type ProphButtonVariant =
  | "primary"        // DESIGN.md §Buttons — Action Blue pill, the brand action
  | "secondary-pill" // ghost pill: transparent fill, blue border + text
  | "dark-utility"   // ink rect, 8px radius — for use against light/parchment, NOT against surface-black
  | "pearl-capsule"  // surface-pearl, 11px radius — secondary on cards
  | "store-hero"     // larger primary, weight 300 (the rare 300)
  | "icon-circular"; // 44x44 translucent chip over photography

export type ProphButtonSize = "default" | "lg";

interface ProphButtonBaseProps {
  variant?: ProphButtonVariant;
  size?: ProphButtonSize;
  className?: string;
  children?: ReactNode;
}

/*
 * DESIGN.md §"Do's" — active/press state across every variant: scale(0.95).
 * Focus ring: 2px solid var(--proph-color-primary-focus).
 * No shadow on any button: shadow is reserved for product imagery.
 *
 * dark-utility note: DESIGN.md §Buttons describes this variant for "global nav
 * actions" but its ink fill (#1d1d1f) reads poorly on a surface-black nav.
 * Use it on light or parchment surfaces only; for nav buttons against pure
 * black, prefer ProphLinkButton with the primary-on-dark accent or a plain
 * text link.
 */
const variantClasses: Record<ProphButtonVariant, string> = {
  primary: cn(
    "bg-proph-primary text-proph-on-primary",
    "rounded-proph-pill px-[22px] py-[11px]",
    "font-text text-[17px] leading-[1.47] tracking-[-0.374px]",
  ),
  "secondary-pill": cn(
    "bg-transparent text-proph-primary",
    "border border-proph-primary",
    "rounded-proph-pill px-[22px] py-[11px]",
    "font-text text-[17px] leading-[1.47] tracking-[-0.374px]",
  ),
  "dark-utility": cn(
    "bg-proph-ink text-proph-on-dark",
    "rounded-proph-sm px-[15px] py-[8px]",
    "font-text text-[14px] font-normal leading-[1.29] tracking-[-0.224px]",
  ),
  "pearl-capsule": cn(
    "bg-proph-surface-pearl text-proph-ink-muted-80",
    "border-[3px] border-proph-divider-soft",
    "rounded-proph-md px-[14px] py-[8px]",
    "font-text text-[14px] leading-[1.43] tracking-[-0.224px]",
  ),
  "store-hero": cn(
    "bg-proph-primary text-proph-on-primary",
    "rounded-proph-pill px-[28px] py-[14px]",
    "font-text text-[18px] font-light leading-none",
  ),
  "icon-circular": cn(
    "bg-proph-surface-chip-translucent text-proph-ink",
    "rounded-proph-full",
    "h-11 w-11 inline-flex items-center justify-center",
  ),
};

const baseClasses = cn(
  "inline-flex items-center justify-center select-none",
  "transition-transform duration-150 ease-out",
  "active:scale-[0.95]",
  "focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2",
  "focus-visible:outline-proph-primary-focus",
);

const buttonOnlyClasses = "disabled:cursor-not-allowed disabled:text-proph-ink-muted-48 disabled:opacity-100";

function resolveClasses(variant: ProphButtonVariant, size: ProphButtonSize, className?: string) {
  const sizeOverride = size === "lg" && variant === "primary" ? "px-[28px] py-[14px] text-[18px] font-light" : "";
  return cn(baseClasses, variantClasses[variant], sizeOverride, className);
}

export type ProphButtonProps = ProphButtonBaseProps & ButtonHTMLAttributes<HTMLButtonElement>;

export const ProphButton = forwardRef<HTMLButtonElement, ProphButtonProps>(
  ({ variant = "primary", size = "default", className, type, ...props }, ref) => (
    <button
      ref={ref}
      type={type ?? "button"}
      className={cn(resolveClasses(variant, size, className), buttonOnlyClasses)}
      {...props}
    />
  ),
);
ProphButton.displayName = "ProphButton";

/*
 * Anchor counterpart — same visual language, but for navigation. Use whenever
 * the action's effect is "go somewhere"; nesting <a> inside <button> (or vice
 * versa) is invalid HTML and breaks focus / right-click semantics.
 */
export type ProphLinkButtonProps = ProphButtonBaseProps &
  AnchorHTMLAttributes<HTMLAnchorElement> & { href: string };

export const ProphLinkButton = forwardRef<HTMLAnchorElement, ProphLinkButtonProps>(
  ({ variant = "primary", size = "default", className, ...props }, ref) => (
    <a ref={ref} className={resolveClasses(variant, size, className)} {...props} />
  ),
);
ProphLinkButton.displayName = "ProphLinkButton";
