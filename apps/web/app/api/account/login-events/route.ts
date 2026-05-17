import { NextResponse } from "next/server";
import { fetchSelfLoginEvents } from "@/lib/login-events";
import { getUpstreamSessionToken } from "@/lib/session";

export async function GET() {
  const upstreamSessionToken = await getUpstreamSessionToken();
  if (!upstreamSessionToken) {
    return NextResponse.json({ error: "upstream_session_missing" }, { status: 401 });
  }

  const events = await fetchSelfLoginEvents(upstreamSessionToken, { limit: 20 });
  return NextResponse.json({ events });
}
