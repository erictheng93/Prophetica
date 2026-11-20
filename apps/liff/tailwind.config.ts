import type { Config } from "tailwindcss";

// Mirrors apps/web/tailwind.config.ts. LIFF runs in LINE's in-app webview
// (narrow vertical viewport), so layouts that consume this config should
// be mobile-first; the token map itself is identical to web/admin.
const config: Config = {
  content: [
    "./app/**/*.{ts,tsx,mdx}",
    "./components/**/*.{ts,tsx}",
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
          "gold-primary": "var(--proph-color-gold-primary, #D4A857)",
          "gold-warm": "var(--proph-color-gold-warm, #F0D78C)",
          "gold-deep": "var(--proph-color-gold-deep, #A68340)",
          "gold-antique": "var(--proph-color-gold-antique, #8C6D2D)",
          "black-velvet": "var(--proph-color-black-velvet, #0A0808)",
          "black-stage": "var(--proph-color-black-stage, #000000)",
          "panel-elevated": "var(--proph-color-panel-elevated, #141010)",
          pearl: "var(--proph-color-pearl, #EDE6D8)",
          "smoke-gold": "var(--proph-color-smoke-gold, #C9B98F)",
          mist: "var(--proph-color-mist, #EAE4D9)",
        },
      },
      borderRadius: {
        "proph-none": "var(--proph-radius-none)",
        "proph-xs": "var(--proph-radius-xs)",
        "proph-sm": "var(--proph-radius-sm)",
        "proph-md": "var(--proph-radius-md)",
        "proph-lg": "var(--proph-radius-lg)",
        "proph-xl": "var(--proph-radius-xl, 24px)",
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
        "proph-card":
          "0 18px 44px rgba(10, 8, 8, 0.08), 0 2px 8px rgba(10, 8, 8, 0.04)",
        "proph-button":
          "0 4px 14px rgba(212, 168, 87, 0.28), inset 0 1px 0 rgba(255, 255, 255, 0.25)",
        "proph-gold-cta":
          "inset 0 1px 0 rgba(255, 240, 200, 0.72), inset 0 -1px 0 rgba(80, 50, 0, 0.5), 0 0 32px rgba(212, 168, 87, 0.34)",
      },
    },
  },
  plugins: [],
};

export default config;
