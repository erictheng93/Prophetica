import type { NextRequest } from "next/server";
import { getViewer } from "@/lib/session";
import {
  proxyMemberCreateRequest,
  proxyMemberListRequest,
} from "@/lib/member-management-bff";

export async function GET() {
  return proxyMemberListRequest({
    viewer: await getViewer(),
  });
}

export async function POST(request: NextRequest) {
  let payload: unknown;
  try {
    payload = await request.json();
  } catch {
    return Response.json({ error: "invalid_json" }, { status: 400 });
  }

  return proxyMemberCreateRequest({
    viewer: await getViewer(),
    request: payload,
  });
}
