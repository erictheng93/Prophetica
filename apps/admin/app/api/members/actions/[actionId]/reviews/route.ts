import type { NextRequest } from "next/server";
import { proxyMemberActionReviewRequest } from "@/lib/member-management-bff";
import { getViewer } from "@/lib/session";

export async function POST(
  request: NextRequest,
  { params }: { params: Promise<{ actionId: string }> },
) {
  let payload: unknown;
  try {
    payload = await request.json();
  } catch {
    return Response.json({ error: "invalid_json" }, { status: 400 });
  }

  const { actionId } = await params;
  return proxyMemberActionReviewRequest({
    viewer: await getViewer(),
    actionId,
    request: payload,
  });
}
