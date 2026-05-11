import type { NextRequest } from "next/server";
import { proxyToGateway } from "../../../proxy";

interface RouteContext {
  params: Promise<{ id: string }>;
}

export async function POST(request: NextRequest, context: RouteContext) {
  const { id } = await context.params;
  return proxyToGateway(
    "POST",
    `/api/internal/admin/report-jobs/${encodeURIComponent(id)}/export-deletions`,
    await request.text(),
  );
}
