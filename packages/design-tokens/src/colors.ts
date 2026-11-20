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
  // Bumped 2026-05-06 from #7a7a7a → #595959 to clear WCAG AA 4.5:1 at 12px
  // body text sizes. Token semantics (a "muted" gray for kickers / labels /
  // legal fine-print) preserved; the original 48%-ink-mix derivation no
  // longer holds but the hex is what consumers depend on.
  "ink-muted-48": "#595959",

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
  // Bumped 2026-05-06 from #a86b00 → #8a5800 to clear WCAG AA 4.5:1 at 12px.
  "feedback-warning": "#8a5800",
  "feedback-success": "#1f7a3f",
} as const;

export type ColorToken = keyof typeof colors;

/**
 * Black Gold Luxury color system — design/BlackGoldLuxuryDesign.md
 */
export const blackGoldColors = {
  goldPrimary: "#D4A857",
  goldWarm: "#F0D78C",
  goldDeep: "#8B6914",
  goldAntique: "#B8860B",
  blackVelvet: "#0A0808",
  blackStage: "#000000",
  panelElevated: "#141010",
  pearl: "#EDE6D8",
  smokeGold: "#C9B98F",
  mist: "#8A8378",
} as const;

export type BlackGoldColorToken = keyof typeof blackGoldColors;
