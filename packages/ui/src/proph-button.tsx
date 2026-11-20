import { forwardRef, type AnchorHTMLAttributes, type ReactNode } from "react";
import { cn } from "./cn";

type ProphLinkButtonVariant =
  | "primary" // DESIGN.md §Buttons — Action Blue pill, the brand action
  | "secondary-pill" // ghost pill: transparent fill, blue border + text (light surfaces)
  | "secondary-pill-on-dark" // ghost pill for dark tiles — primary-on-dark Sky Blue
  | "dark-utility" // ink rect, 8px radius — for use against light/parchment, NOT against surface-black
  | "pearl-capsule" // surface-pearl, 11px radius — secondary on cards
  | "store-hero" // larger primary, weight 300 (the rare 300)
  | "icon-circular" // 44x44 translucent chip over photography
  | "gold-primary" // Black Gold Luxury — soft champagne gold pill with warm highlight
  | "gold-ghost" // Black Gold Luxury — antique gold border with pearl text
  | "glass"; // Ultra-refined frosted glass pill with subtle border

type ProphLinkButtonSize = "default" | "lg" | "sm";

interface ProphLinkButtonBaseProps {
  variant?: ProphLinkButtonVariant;
  size?: ProphLinkButtonSize;
  className?: string;
  children?: ReactNode;
}

/*
 * DESIGN.md §"Do's" — active/press state across every variant: scale(0.95).
 * Focus ring: 2px solid var(--proph-color-primary-focus).
 */
const variantClasses: Record<ProphLinkButtonVariant, string> = {
  primary: cn(
    "bg-proph-primary text-proph-on-primary",
    "rounded-proph-pill px-[22px] py-[11px]",
    "font-text text-[17px] leading-[1.47] tracking-[-0.374px]",
    "hover:brightness-105 shadow-[0_2px_12px_rgba(0,102,204,0.2)]",
  ),
  "secondary-pill": cn(
    "bg-transparent text-proph-primary",
    "border border-proph-primary",
    "rounded-proph-pill px-[22px] py-[11px]",
    "font-text text-[17px] leading-[1.47] tracking-[-0.374px]",
    "hover:bg-proph-primary/5",
  ),
  "secondary-pill-on-dark": cn(
    "bg-transparent text-proph-primary-on-dark",
    "border border-proph-primary-on-dark",
    "rounded-proph-pill px-[22px] py-[11px]",
    "font-text text-[17px] leading-[1.47] tracking-[-0.374px]",
    "hover:bg-proph-primary-on-dark/10",
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
  "gold-primary": cn(
    "bg-gradient-to-b from-[#F0D78C] via-[#D4A857] to-[#A68340] text-black font-semibold",
    "rounded-proph-pill px-[24px] py-[12px]",
    "font-text text-[16px] leading-none",
    "shadow-[inset_0_1px_0_rgba(255,240,200,0.72),inset_0_-1px_0_rgba(80,50,0,0.5),0_0_24px_rgba(212,168,87,0.32)]",
    "hover:brightness-105",
  ),
  "gold-ghost": cn(
    "bg-transparent text-[#EDE6D8] border border-[#D4A857]/60",
    "rounded-proph-pill px-[24px] py-[12px]",
    "font-text text-[16px] leading-none",
    "hover:bg-[#D4A857]/10 hover:text-[#F0D78C] hover:border-[#D4A857]",
  ),
  glass: cn(
    "bg-white/10 dark:bg-black/30 backdrop-blur-md text-proph-ink dark:text-proph-on-dark border border-white/20 dark:border-white/10",
    "rounded-proph-pill px-[20px] py-[10px]",
    "font-text text-[15px] leading-normal",
    "hover:bg-white/20 dark:hover:bg-white/15",
  ),
};

const baseClasses = cn(
  "inline-flex items-center justify-center select-none cursor-pointer",
  "transition-all duration-150 ease-out",
  "active:scale-[0.96]",
  "focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2",
  "focus-visible:outline-proph-primary-focus",
);

function resolveClasses(
  variant: ProphLinkButtonVariant,
  size: ProphLinkButtonSize,
  className?: string,
) {
  let sizeOverride = "";
  if (size === "lg") {
    sizeOverride = "px-[28px] py-[14px] text-[18px]";
  } else if (size === "sm") {
    sizeOverride = "px-[14px] py-[6px] text-[13px]";
  }
  return cn(baseClasses, variantClasses[variant], sizeOverride, className);
}

export type ProphLinkButtonProps = ProphLinkButtonBaseProps &
  AnchorHTMLAttributes<HTMLAnchorElement> & { href: string };

export const ProphLinkButton = forwardRef<
  HTMLAnchorElement,
  ProphLinkButtonProps
>(({ variant = "primary", size = "default", className, ...props }, ref) => (
  <a
    ref={ref}
    className={resolveClasses(variant, size, className)}
    {...props}
  />
));
ProphLinkButton.displayName = "ProphLinkButton";

export type ProphButtonProps = ProphLinkButtonBaseProps &
  React.ButtonHTMLAttributes<HTMLButtonElement>;

export const ProphButton = forwardRef<HTMLButtonElement, ProphButtonProps>(
  (
    {
      variant = "primary",
      size = "default",
      className,
      type = "button",
      ...props
    },
    ref,
  ) => (
    <button
      ref={ref}
      type={type}
      className={resolveClasses(variant, size, className)}
      {...props}
    />
  ),
);
ProphButton.displayName = "ProphButton";
