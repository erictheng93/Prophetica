import { forwardRef } from "react";
import { ProphLinkButton, type ProphLinkButtonProps } from "../proph-button";

export type KycEntryButtonProps = Omit<ProphLinkButtonProps, "children"> & {
  label?: string;
};

export const KycEntryButton = forwardRef<
  HTMLAnchorElement,
  KycEntryButtonProps
>(({ label = "開始驗證", variant = "primary", href, ...props }, ref) => (
  <ProphLinkButton ref={ref} variant={variant} href={href} {...props}>
    {label}
  </ProphLinkButton>
));
KycEntryButton.displayName = "KycEntryButton";
