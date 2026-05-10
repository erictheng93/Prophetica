import { ImageResponse } from "next/og";

/*
 * Browser favicon (32x32). DESIGN.md mandates the Action Blue (#0066cc)
 * as the only brand-level accent — render a single uppercase "P" centered
 * in the surface so the favicon stays recognizable even at 16px.
 *
 * We rely on Next.js's file-based icon convention: this default export
 * is hit at /icon and surfaced as a <link rel="icon"> automatically.
 */

export const runtime = "edge";
export const size = { width: 32, height: 32 };
export const contentType = "image/png";

export default function Icon() {
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
          fontSize: 22,
          fontWeight: 600,
          letterSpacing: "-0.5px",
        }}
      >
        P
      </div>
    ),
    { ...size },
  );
}
