import type { NextRequest } from "next/server";
import { proxyToGateway } from "../../../proxy";

interface RouteContext {
  params: Promise<{ id: string }>;
}

export async function GET(request: NextRequest, context: RouteContext) {
  const { id } = await context.params;
  const format = request.nextUrl.searchParams.get("format") ?? "csv";
  return proxyToGateway(
    "GET",
    `/api/internal/admin/report-jobs/${encodeURIComponent(id)}/exports/${encodeURIComponent(format)}`,
  );
}
