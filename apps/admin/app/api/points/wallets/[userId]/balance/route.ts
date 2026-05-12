import { getViewer } from "@/lib/session";
import { proxyPointsBalanceRequest } from "@/lib/points-bff";

interface RouteContext {
  params: Promise<{ userId: string }>;
}

export async function GET(_request: Request, context: RouteContext) {
  const { userId } = await context.params;
  return proxyPointsBalanceRequest({
    viewer: await getViewer(),
    userId,
  });
}
