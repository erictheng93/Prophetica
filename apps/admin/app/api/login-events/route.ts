import { NextResponse } from "next/server";
import { fetchAdminLoginEvents } from "@/lib/login-events-bff";
import { canAccess } from "@/lib/rbac";
import { getViewer } from "@/lib/session";
import { getUpstreamSessionToken } from "@/lib/upstream-session";

export async function GET(request: Request) {
  const viewer = await getViewer();
  if (!viewer || !canAccess("/security/login-events", viewer.role)) {
    return NextResponse.json({ error: "forbidden" }, { status: 403 });
  }
  const upstreamSessionToken = await getUpstreamSessionToken();
  if (!upstreamSessionToken) {
    return NextResponse.json({ error: "upstream_session_missing" }, { status: 401 });
  }

  const url = new URL(request.url);
  const events = await fetchAdminLoginEvents(upstreamSessionToken, {
    limit: Number(url.searchParams.get("limit") ?? 50),
    filters: {
      principal_type: url.searchParams.get("principal_type") ?? undefined,
      principal_id: url.searchParams.get("principal_id") ?? undefined,
      user_id: url.searchParams.get("user_id") ?? undefined,
      ip_address: url.searchParams.get("ip_address") ?? undefined,
      event_type: url.searchParams.get("event_type") ?? undefined,
      from_epoch_seconds: url.searchParams.get("from_epoch_seconds") ?? undefined,
      to_epoch_seconds: url.searchParams.get("to_epoch_seconds") ?? undefined,
    },
  });
  return NextResponse.json({ events });
}
