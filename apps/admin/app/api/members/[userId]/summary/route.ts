import { getViewer } from "@/lib/session";
import { proxyMemberSummaryRequest } from "@/lib/member-management-bff";

interface RouteContext {
  params: Promise<{ userId: string }>;
}

export async function GET(_request: Request, context: RouteContext) {
  const { userId } = await context.params;
  return proxyMemberSummaryRequest({
    viewer: await getViewer(),
    userId,
  });
}
