import { getViewer } from "@/lib/session";
import { proxyKycDocumentRequest } from "@/lib/kyc-documents-bff";

interface RouteContext {
  params: Promise<{ userId: string }>;
}

export async function GET(_request: Request, context: RouteContext) {
  const viewer = await getViewer();
  if (!viewer || (viewer.role !== "admin" && viewer.role !== "manager")) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }
  const { userId } = await context.params;
  const enc = encodeURIComponent(userId);
  return proxyKycDocumentRequest(viewer, {
    userId,
    adminPath: `/api/internal/admin/members/${enc}/kyc-documents`,
  });
}
