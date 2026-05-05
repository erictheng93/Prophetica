/**
 * Spacing tokens — design/DESIGN.md §Layout.
 * Base unit 8px; section padding 80px is the tile-rhythm anchor.
 */
export const spacing = {
  xxs: "4px",
  xs: "8px",
  sm: "12px",
  md: "17px",
  lg: "24px",
  xl: "32px",
  xxl: "48px",
  section: "80px",
} as const;

export type SpacingToken = keyof typeof spacing;
