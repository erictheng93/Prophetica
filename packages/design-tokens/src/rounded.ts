/**
 * Border-radius scale — design/DESIGN.md §Shapes.
 * `pill` and `full` resolve to the same 9999px; the alias preserves intent
 * (pill = capsule CTA, full = circle).
 */
export const rounded = {
  none: "0px",
  xs: "5px",
  sm: "8px",
  md: "11px",
  lg: "18px",
  pill: "9999px",
  full: "9999px",
} as const;

export type RoundedToken = keyof typeof rounded;
