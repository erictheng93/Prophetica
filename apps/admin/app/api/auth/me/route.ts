import { NextResponse } from "next/server";
import { getViewer } from "@/lib/session";

export async function GET() {
  const viewer = await getViewer();
  if (!viewer) {
    return NextResponse.json({ authenticated: false }, { status: 401 });
  }
  return NextResponse.json({
    authenticated: true,
    sub: viewer.sub,
    role: viewer.role,
    affiliateId: viewer.affiliateId ?? null,
  });
}
