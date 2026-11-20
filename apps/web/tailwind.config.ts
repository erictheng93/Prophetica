import type { Config } from "tailwindcss";

/*
 * Tailwind theme reads from CSS custom properties published by
 * `@prophetica/design-tokens/css`. This keeps `tokens.css` as the runtime
 * source of truth (theming, dark-mode swaps, Kumo bridge variables) while
 * Tailwind utility class generation stays declarative here.
 */
const config: Config = {
  content: [
    "./app/**/*.{ts,tsx,mdx}",
    "./components/**/*.{ts,tsx}",
    "./lib/**/*.{ts,tsx}",
    // Workspace primitives — Tailwind must scan these to emit utility classes
    // referenced inside @prophetica/ui components.
    "../../packages/ui/src/**/*.{ts,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        proph: {
          primary: "var(--proph-color-primary)",
          "primary-focus": "var(--proph-color-primary-focus)",
          "primary-on-dark": "var(--proph-color-primary-on-dark)",
          ink: "var(--proph-color-ink)",
          body: "var(--proph-color-body)",
          "body-on-dark": "var(--proph-color-body-on-dark)",
          "body-muted": "var(--proph-color-body-muted)",
          "ink-muted-80": "var(--proph-color-ink-muted-80)",
          "ink-muted-48": "var(--proph-color-ink-muted-48)",
          "divider-soft": "var(--proph-color-divider-soft)",
          hairline: "var(--proph-color-hairline)",
          canvas: "var(--proph-color-canvas)",
          "canvas-parchment": "var(--proph-color-canvas-parchment)",
          "surface-pearl": "var(--proph-color-surface-pearl)",
          "surface-tile-1": "var(--proph-color-surface-tile-1)",
          "surface-tile-2": "var(--proph-color-surface-tile-2)",
          "surface-tile-3": "var(--proph-color-surface-tile-3)",
          "surface-black": "var(--proph-color-surface-black)",
          "surface-chip-translucent":
            "var(--proph-color-surface-chip-translucent)",
          "on-primary": "var(--proph-color-on-primary)",
          "on-dark": "var(--proph-color-on-dark)",
          "feedback-error": "var(--proph-color-feedback-error)",
          "feedback-warning": "var(--proph-color-feedback-warning)",
          "feedback-success": "var(--proph-color-feedback-success)",
          // Black Gold & Luxury accents
          "gold-primary": "var(--proph-gold-primary)",
          "gold-warm": "var(--proph-gold-warm)",
          "gold-deep": "var(--proph-gold-deep)",
          "gold-antique": "var(--proph-gold-antique)",
          "black-velvet": "var(--proph-black-velvet)",
          "black-stage": "var(--proph-black-stage)",
          "panel-elevated": "var(--proph-panel-elevated)",
          pearl: "var(--proph-pearl)",
          "smoke-gold": "var(--proph-smoke-gold)",
          mist: "var(--proph-mist)",
        },
      },
      borderRadius: {
        "proph-none": "var(--proph-radius-none)",
        "proph-xs": "var(--proph-radius-xs)",
        "proph-sm": "var(--proph-radius-sm)",
        "proph-md": "var(--proph-radius-md)",
        "proph-lg": "var(--proph-radius-lg)",
        "proph-pill": "var(--proph-radius-pill)",
        "proph-full": "var(--proph-radius-full)",
      },
      fontFamily: {
        display: ["var(--proph-font-display)"],
        text: ["var(--proph-font-text)"],
      },
      maxWidth: {
        proph: "1440px",
      },
      boxShadow: {
        "proph-product": "var(--proph-shadow-product)",
        "proph-card": "var(--proph-shadow-card)",
        "proph-gold-cta": "var(--proph-shadow-gold-cta)",
      },
    },
  },
  plugins: [],
};

export default config;
