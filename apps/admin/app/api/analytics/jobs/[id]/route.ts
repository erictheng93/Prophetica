import { proxyToGateway } from "../../proxy";

interface RouteContext {
  params: Promise<{ id: string }>;
}

export async function GET(_request: Request, context: RouteContext) {
  const { id } = await context.params;
  return proxyToGateway("GET", `/api/internal/admin/report-jobs/${encodeURIComponent(id)}`);
}
