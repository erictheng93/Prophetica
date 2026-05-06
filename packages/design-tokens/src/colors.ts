/**
 * Color tokens — exact values from design/DESIGN.md §Colors.
 * Adding new hex literals outside this file violates the design system.
 */
export const colors = {
  // Brand & accent
  primary: "#0066cc",
  "primary-focus": "#0071e3",
  "primary-on-dark": "#2997ff",

  // Text
  ink: "#1d1d1f",
  body: "#1d1d1f",
  "body-on-dark": "#ffffff",
  "body-muted": "#cccccc",
  "ink-muted-80": "#333333",
  "ink-muted-48": "#7a7a7a",

  // Hairlines
  "divider-soft": "#f0f0f0",
  hairline: "#e0e0e0",

  // Surface
  canvas: "#ffffff",
  "canvas-parchment": "#f5f5f7",
  "surface-pearl": "#fafafc",
  "surface-tile-1": "#272729",
  "surface-tile-2": "#2a2a2c",
  "surface-tile-3": "#252527",
  "surface-black": "#000000",
  "surface-chip-translucent": "#d2d2d7",

  // On-color
  "on-primary": "#ffffff",
  "on-dark": "#ffffff",

  // Feedback (form validation / system messages).
  // Tuned slightly below Apple's iOS systemRed/systemGreen so they pair with
  // ink (#1d1d1f) rather than competing for attention. These are NOT brand
  // accents — they exist exclusively for form errors, status messages, and
  // the rare destructive-action confirmation. Do not use them for marketing.
  "feedback-error": "#c92020",
  "feedback-warning": "#a86b00",
  "feedback-success": "#1f7a3f",
} as const;

export type ColorToken = keyof typeof colors;
