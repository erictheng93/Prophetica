/**
 * Typography tokens — exact values from design/DESIGN.md §Typography.
 *
 * Display sizes use SF Pro Display; body / UI uses SF Pro Text.
 * On non-Apple platforms the system stack falls through to Inter (see DESIGN.md
 * §"Note on Font Substitutes"); when Inter is the resolved face, callers should
 * tighten letter-spacing by an additional -0.01em on display sizes.
 */
const display = "SF Pro Display, system-ui, -apple-system, BlinkMacSystemFont, Inter, sans-serif";
const text = "SF Pro Text, system-ui, -apple-system, BlinkMacSystemFont, Inter, sans-serif";

export const typography = {
  "hero-display": { fontFamily: display, fontSize: "56px", fontWeight: 600, lineHeight: 1.07, letterSpacing: "-0.28px" },
  "display-lg":   { fontFamily: display, fontSize: "40px", fontWeight: 600, lineHeight: 1.10, letterSpacing: "0" },
  "display-md":   { fontFamily: text,    fontSize: "34px", fontWeight: 600, lineHeight: 1.47, letterSpacing: "-0.374px" },
  "lead":         { fontFamily: display, fontSize: "28px", fontWeight: 400, lineHeight: 1.14, letterSpacing: "0.196px" },
  "lead-airy":    { fontFamily: text,    fontSize: "24px", fontWeight: 300, lineHeight: 1.50, letterSpacing: "0" },
  "tagline":      { fontFamily: display, fontSize: "21px", fontWeight: 600, lineHeight: 1.19, letterSpacing: "0.231px" },
  "body-strong":  { fontFamily: text,    fontSize: "17px", fontWeight: 600, lineHeight: 1.24, letterSpacing: "-0.374px" },
  "body":         { fontFamily: text,    fontSize: "17px", fontWeight: 400, lineHeight: 1.47, letterSpacing: "-0.374px" },
  "dense-link":   { fontFamily: text,    fontSize: "17px", fontWeight: 400, lineHeight: 2.41, letterSpacing: "0" },
  "caption":      { fontFamily: text,    fontSize: "14px", fontWeight: 400, lineHeight: 1.43, letterSpacing: "-0.224px" },
  "caption-strong": { fontFamily: text,  fontSize: "14px", fontWeight: 600, lineHeight: 1.29, letterSpacing: "-0.224px" },
  "button-large": { fontFamily: text,    fontSize: "18px", fontWeight: 300, lineHeight: 1.00, letterSpacing: "0" },
  "button-utility": { fontFamily: text,  fontSize: "14px", fontWeight: 400, lineHeight: 1.29, letterSpacing: "-0.224px" },
  "fine-print":   { fontFamily: text,    fontSize: "12px", fontWeight: 400, lineHeight: 1.00, letterSpacing: "-0.12px" },
  "micro-legal":  { fontFamily: text,    fontSize: "10px", fontWeight: 400, lineHeight: 1.30, letterSpacing: "-0.08px" },
  "nav-link":     { fontFamily: text,    fontSize: "12px", fontWeight: 400, lineHeight: 1.00, letterSpacing: "-0.12px" },
} as const;

export type TypographyToken = keyof typeof typography;
export const fontFamilies = { display, text } as const;
