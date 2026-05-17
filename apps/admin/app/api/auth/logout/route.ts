import { NextResponse } from "next/server";
import { clearSessionCookie } from "@/lib/session";
import { clearUpstreamSessionCookie } from "@/lib/upstream-session";

export async function POST() {
  await clearSessionCookie();
  await clearUpstreamSessionCookie();
  return NextResponse.json({ ok: true });
}
