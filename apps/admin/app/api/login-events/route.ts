import { NextResponse } from "next/server";
import { fetchAdminLoginEvents } from "@/lib/login-events-bff";
import { canAccess } from "@/lib/rbac";
import { getViewer } from "@/lib/session";
import { getUpstreamSessionToken } from "@/lib/upstream-session";

export async function GET() {
  const viewer = await getViewer();
  if (!viewer || !canAccess("/security/login-events", viewer.role)) {
    return NextResponse.json({ error: "forbidden" }, { status: 403 });
  }
  const upstreamSessionToken = await getUpstreamSessionToken();
  if (!upstreamSessionToken) {
    return NextResponse.json({ error: "upstream_session_missing" }, { status: 401 });
  }

  const events = await fetchAdminLoginEvents(upstreamSessionToken, { limit: 50 });
  return NextResponse.json({ events });
}
