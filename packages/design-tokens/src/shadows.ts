/**
 * Elevation tokens — design/DESIGN.md §"Elevation & Depth".
 *
 * Apple's system has exactly ONE drop-shadow, reserved for product imagery.
 * `product` is the only legal shadow on UI surfaces in this design system.
 * Cards, buttons, and text MUST NOT carry a shadow.
 */
export const shadows = {
  none: "none",
  product: "rgba(0, 0, 0, 0.22) 3px 5px 30px 0",
} as const;

/**
 * Frosted-glass surface stack — design/DESIGN.md §"Decorative Depth".
 * Used by sub-nav and floating sticky bar. Backdrop-filter is the depth signal,
 * not a drop-shadow.
 */
export const frosted = {
  // saturate(180%) is the Apple-typical baseline; blur(20px) is the documented default.
  backdropFilter: "saturate(180%) blur(20px)",
} as const;

export type ShadowToken = keyof typeof shadows;
