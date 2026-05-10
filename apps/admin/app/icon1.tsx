import { ImageResponse } from "next/og";

export const runtime = "edge";
export const size = { width: 512, height: 512 };
export const contentType = "image/png";

export default function Icon512() {
  return new ImageResponse(
    <div
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: "#1d1d1f",
        color: "#ffffff",
        fontSize: 292,
        fontWeight: 600,
        letterSpacing: "-8px",
      }}
    >
      PA
    </div>,
    { ...size },
  );
}
