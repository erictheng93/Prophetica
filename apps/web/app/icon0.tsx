import { ImageResponse } from "next/og";

// Manifest 192x192 icon — matches /icon0 in app/manifest.ts.
// Slightly larger letter so the glyph keeps optical weight when the OS
// downscales to launcher tile sizes (Android low-density screens get ~120dp).

export const runtime = "edge";
export const size = { width: 192, height: 192 };
export const contentType = "image/png";

export default function Icon192() {
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
          fontSize: 132,
          fontWeight: 600,
          letterSpacing: "-3px",
        }}
      >
        P
      </div>
    ),
    { ...size },
  );
}
