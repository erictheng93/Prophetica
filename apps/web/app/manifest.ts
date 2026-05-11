import type { MetadataRoute } from "next";

/*
 * SPEC-017 §6 web app metadata.
 *
 * The Apple-style design system mandates exactly one accent color
 * (Action Blue #0066cc, see DESIGN.md §Colors); we reuse it as the PWA
 * theme color so Android Chrome / iOS Safari paint the URL bar with the
 * same hue users see on the page primary CTAs.
 *
 * background_color is the parchment off-white (#f5f5f7) used by the site
 * footer / sub-nav so the splash screen does not flash a different shade
 * before the first paint.
 *
 * Icons are static PNGs in public/ so the Cloudflare OpenNext adapter does
 * not need unsupported Edge runtime icon routes. The 512-square is also
 * marked maskable so Android can crop it into adaptive icon shapes.
 */

export default function manifest(): MetadataRoute.Manifest {
  return {
    name: "Prophetica",
    short_name: "Prophetica",
    description: "Event-prediction experience built on Apple-style design.",
    start_url: "/",
    scope: "/",
    display: "standalone",
    orientation: "portrait-primary",
    background_color: "#f5f5f7",
    theme_color: "#0066cc",
    lang: "zh-Hant",
    icons: [
      {
        src: "/icon-192.png",
        sizes: "192x192",
        type: "image/png",
        purpose: "any",
      },
      {
        src: "/icon-512.png",
        sizes: "512x512",
        type: "image/png",
        purpose: "any",
      },
      {
        src: "/icon-512.png",
        sizes: "512x512",
        type: "image/png",
        purpose: "maskable",
      },
    ],
  };
}
