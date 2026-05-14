import type { NextRequest } from "next/server";
import { getViewer } from "@/lib/session";
import {
  proxyMemberDeleteRequest,
  proxyMemberUpdateRequest,
} from "@/lib/member-management-bff";

interface RouteContext {
  params: Promise<{ userId: string }>;
}

export async function PATCH(request: NextRequest, context: RouteContext) {
  const { userId } = await context.params;
  let payload: unknown;
  try {
    payload = await request.json();
  } catch {
    return Response.json({ error: "invalid_json" }, { status: 400 });
  }

  return proxyMemberUpdateRequest({
    viewer: await getViewer(),
    userId,
    request: payload,
  });
}

export async function DELETE(_request: NextRequest, context: RouteContext) {
  const { userId } = await context.params;
  return proxyMemberDeleteRequest({
    viewer: await getViewer(),
    userId,
  });
}
