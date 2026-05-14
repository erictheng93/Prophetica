import type { NextRequest } from "next/server";
import { getViewer } from "@/lib/session";
import { proxyMemberActionRequest } from "@/lib/member-management-bff";

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
