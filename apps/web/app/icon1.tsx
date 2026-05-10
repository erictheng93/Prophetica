import { ImageResponse } from "next/og";

/*
 * Manifest 512x512 icon (also used as the maskable variant — see
 * app/manifest.ts).
 *
 * Android adaptive-icon mask reserves the outer ~10% of the canvas for
 * launcher cropping. The "P" lives inside the central 40% so every mask
 * shape (squircle, circle, teardrop) keeps the full glyph visible.
 */

export const runtime = "edge";
export const size = { width: 512, height: 512 };
export const contentType = "image/png";

export default function Icon512() {
  return new ImageResponse(
    (
      <div
        style={{
          width: "100%",
          height: "100%",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          background: "#0066cc",
          color: "#ffffff",
          fontSize: 320,
          fontWeight: 600,
          letterSpacing: "-8px",
        }}
      >
        P
      </div>
    ),
    { ...size },
  );
}
