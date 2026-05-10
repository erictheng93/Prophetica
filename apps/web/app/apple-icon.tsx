import { ImageResponse } from "next/og";

/*
 * iOS home-screen icon (180x180 is the size iOS Safari requests via
 * <link rel="apple-touch-icon">). iOS does not honor PWA manifest
 * `purpose: maskable`, so we ship a non-masked square that still reads
 * well under iOS's automatic rounded-rectangle clip.
 */

export const runtime = "edge";
export const size = { width: 180, height: 180 };
export const contentType = "image/png";

export default function AppleIcon() {
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
          fontSize: 124,
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
