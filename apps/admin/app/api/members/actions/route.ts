import type { NextRequest } from "next/server";
import { getViewer } from "@/lib/session";
import {
  proxyMemberActionListRequest,
  proxyMemberActionRequest,
} from "@/lib/member-management-bff";

export async function GET(request: NextRequest) {
  return proxyMemberActionListRequest({
    viewer: await getViewer(),
    searchParams: request.nextUrl.searchParams,
  });
}

export async function POST(request: NextRequest) {
  let payload: unknown;
  try {
    payload = await request.json();
  } catch {
    return Response.json({ error: "invalid_json" }, { status: 400 });
  }

  return proxyMemberActionRequest({
    viewer: await getViewer(),
    request: payload,
  });
}
