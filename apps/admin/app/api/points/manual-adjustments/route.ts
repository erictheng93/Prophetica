import type { NextRequest } from "next/server";
import { getViewer } from "@/lib/session";
import { proxyManualPointsAdjustment } from "@/lib/points-bff";

export async function POST(request: NextRequest) {
  let payload: unknown;
  try {
    payload = await request.json();
  } catch {
    return Response.json({ error: "invalid_json" }, { status: 400 });
  }

  return proxyManualPointsAdjustment({
    viewer: await getViewer(),
    request: payload,
  });
}
